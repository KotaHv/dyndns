use std::{
    borrow::Cow,
    net::{Ipv4Addr, Ipv6Addr},
};

use axum::http::Uri;
use chrono::{NaiveDateTime, Utc};

use diesel::{
    deserialize::FromSql,
    expression::expression_types::NotSelectable,
    prelude::*,
    serialize::{IsNull, ToSql},
    sql_types::Integer,
    sqlite::{Sqlite, SqliteValue},
    AsExpression, FromSqlRow,
};
use serde::{de, Deserialize, Serialize};
use validator::{Validate, ValidationError};

use super::{
    Paginate, {dyndns, history},
};
use crate::{util::get_interfaces, DbConn, Error};

#[repr(i32)]
#[derive(Debug, FromSqlRow, AsExpression, Clone, Copy)]
#[diesel(sql_type = Integer)]
pub enum IpVersion {
    V4 = 1,
    V6 = 2,
    ALL = 3,
}

impl ToSql<Integer, diesel::sqlite::Sqlite> for IpVersion {
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, diesel::sqlite::Sqlite>,
    ) -> diesel::serialize::Result {
        out.set_value(*self as i32);
        Ok(IsNull::No)
    }
}

impl FromSql<Integer, diesel::sqlite::Sqlite> for IpVersion {
    fn from_sql(bytes: SqliteValue<'_, '_, '_>) -> diesel::deserialize::Result<Self> {
        match i32::from_sql(bytes)? {
            1 => Ok(Self::V4),
            2 => Ok(Self::V6),
            3 => Ok(Self::ALL),
            x => Err(format!("Unrecognized variant {}", x).into()),
        }
    }
}

impl Serialize for IpVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(*self as i32)
    }
}

impl<'de> Deserialize<'de> for IpVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = i32::deserialize(deserializer)?;
        match v {
            1 => Ok(Self::V4),
            2 => Ok(Self::V6),
            3 => Ok(Self::ALL),
            _ => Err(de::Error::unknown_field(
                v.to_string().as_str(),
                &["1", "2", "3"],
            )),
        }
    }
}

#[derive(
    Debug, Deserialize, Serialize, Selectable, Queryable, Insertable, AsChangeset, Validate,
)]
#[diesel(table_name = dyndns)]
pub struct DynDNS {
    #[validate(custom = "validate_host")]
    pub server: String,
    #[validate(length(min = 1))]
    pub username: String,
    #[validate(length(min = 1))]
    pub password: String,
    #[validate(custom = "validate_host")]
    pub hostname: String,
    pub ip: IpVersion,
    #[validate(length(min = 1), custom = "validate_interface")]
    pub interface: String,
    #[validate(range(min = 1))]
    pub sleep_interval: i32,
}

fn validate_interface(interface: &str) -> Result<(), ValidationError> {
    let mut error = ValidationError::new("interface");
    let interfaces = get_interfaces();
    match interfaces {
        Ok(interfaces) => {
            if interfaces.get(interface).is_none() {
                error.message = Some(Cow::Owned(format!(
                    "unknown field `{}`, expected {:?}",
                    interface, interfaces
                )));
                return Err(error);
            }
        }
        Err(e) => {
            error.message = Some(Cow::Owned(e.to_string()));
            return Err(error);
        }
    }
    Ok(())
}

fn validate_host(host: &str) -> Result<(), ValidationError> {
    let mut error = ValidationError::new("host");
    let url = host.parse::<Uri>();
    let url = match url {
        Ok(url) => url,
        Err(e) => {
            error.message = Some(Cow::Owned(e.to_string()));
            return Err(error);
        }
    };
    if url.scheme().is_some() || url.path_and_query().is_some() {
        error.message = Some(Cow::Borrowed("only be the url host"));
        return Err(error);
    }
    Ok(())
}

impl DynDNS {
    pub async fn get(conn: &DbConn) -> Result<DynDNS, Error> {
        conn.interact(|conn| dyndns::table.select(DynDNS::as_select()).first(conn))
            .await?
            .map_err(|e| e.into())
    }

    pub async fn get_option(conn: &DbConn) -> Result<Option<DynDNS>, Error> {
        conn.interact(|conn| {
            dyndns::table
                .select(DynDNS::as_select())
                .first(conn)
                .optional()
        })
        .await?
        .map_err(|e| e.into())
    }

    pub async fn create(conn: &DbConn, dyndns: DynDNS) -> Result<DynDNS, Error> {
        conn.interact(|conn| {
            diesel::replace_into(dyndns::table)
                .values(dyndns)
                .returning(DynDNS::as_returning())
                .get_result(conn)
        })
        .await?
        .map_err(|e| e.into())
    }

    pub async fn update(conn: &DbConn, dyndns: DynDNS) -> Result<DynDNS, Error> {
        conn.interact(|conn| {
            diesel::update(dyndns::table)
                .filter(dyndns::id.eq(1))
                .set(dyndns)
                .returning(DynDNS::as_returning())
                .get_result(conn)
        })
        .await?
        .map_err(|e| e.into())
    }

    pub async fn get_sleep_interval(conn: &DbConn) -> Result<i32, Error> {
        conn.interact(|conn| dyndns::table.select(dyndns::sleep_interval).first(conn))
            .await?
            .map_err(|e| e.into())
    }
}

#[derive(Serialize, Selectable, Queryable, Insertable)]
#[diesel(table_name=history)]
pub struct History {
    old_ip: Option<String>,
    new_ip: String,
    version: HistoryIpVersion,
    updated: NaiveDateTime,
}

pub type BoxHistoryOrder =
    Box<dyn BoxableExpression<history::table, Sqlite, SqlType = NotSelectable>>;

impl History {
    pub async fn paginate(
        conn: &DbConn,
        page: usize,
        per_page: i64,
        order: Vec<BoxHistoryOrder>,
    ) -> Result<(Vec<Self>, i64), Error> {
        conn.interact(move |conn| {
            order
                .into_iter()
                .fold(history::table.into_boxed(), |query, o| {
                    query.then_order_by(o)
                })
                .select(Self::as_select())
                .paginate(page as i64)
                .per_page(per_page)
                .load_and_total(conn)
        })
        .await?
        .map_err(|e| e.into())
    }
    pub async fn list(conn: &DbConn, page: usize, per_page: i64) -> Result<Vec<History>, Error> {
        let offset = if per_page < 0 {
            0
        } else {
            (page as i64 - 1) * per_page
        };
        conn.interact(move |conn| {
            history::table
                .select(History::as_select())
                .limit(per_page)
                .offset(offset)
                .load(conn)
        })
        .await?
        .map_err(|e| e.into())
    }
    pub async fn total(conn: &DbConn) -> Result<i64, Error> {
        conn.interact(|conn| history::table.count().first(conn))
            .await?
            .map_err(|e| e.into())
    }

    pub async fn insert_v4(
        conn: &DbConn,
        old_ip: &Option<Ipv4Addr>,
        new_ip: &Ipv4Addr,
    ) -> Result<(), Error> {
        let old_ip = old_ip.as_ref().map(|v| v.to_string());
        let new_ip = new_ip.to_string();
        let version = HistoryIpVersion::V4;
        let h = History {
            old_ip,
            new_ip,
            version,
            updated: Utc::now().naive_utc(),
        };
        conn.interact(|conn| diesel::insert_into(history::table).values(h).execute(conn))
            .await??;
        Ok(())
    }

    pub async fn insert_v6(
        conn: &DbConn,
        old_ip: &Option<Vec<Ipv6Addr>>,
        new_ip: &Vec<Ipv6Addr>,
    ) -> Result<(), Error> {
        let old_ip = old_ip.as_ref().map(|v| {
            v.iter()
                .map(|&x| x.to_string())
                .collect::<Vec<String>>()
                .join(",")
        });
        let new_ip = new_ip
            .iter()
            .map(|&x| x.to_string())
            .collect::<Vec<String>>()
            .join(",");
        let version = HistoryIpVersion::V6;
        let h = History {
            old_ip,
            new_ip,
            version,
            updated: Utc::now().naive_utc(),
        };
        conn.interact(|conn| diesel::insert_into(history::table).values(h).execute(conn))
            .await??;
        Ok(())
    }

    async fn get_new_ip(conn: &DbConn, version: HistoryIpVersion) -> Result<Option<String>, Error> {
        conn.interact(|conn| {
            history::table
                .filter(history::version.eq(version))
                .select(history::new_ip)
                .order(history::id.desc())
                .first(conn)
                .optional()
        })
        .await?
        .map_err(|e| e.into())
    }

    pub async fn get_v4(conn: &DbConn) -> Result<Option<String>, Error> {
        Self::get_new_ip(conn, HistoryIpVersion::V4).await
    }

    pub async fn get_v6(conn: &DbConn) -> Result<Option<(Option<String>, String)>, Error> {
        Ok(Self::get_current(conn, HistoryIpVersion::V6)
            .await?
            .map(|history| (history.old_ip, history.new_ip)))
    }

    pub async fn get_current(
        conn: &DbConn,
        version: HistoryIpVersion,
    ) -> Result<Option<History>, Error> {
        conn.interact(|conn| {
            history::table
                .filter(history::version.eq(version))
                .select(Self::as_select())
                .order(history::id.desc())
                .first(conn)
                .optional()
        })
        .await?
        .map_err(|e| e.into())
    }
}

#[repr(i32)]
#[derive(Debug, FromSqlRow, AsExpression, Clone, Deserialize, Serialize)]
#[diesel(sql_type = Integer)]
pub enum HistoryIpVersion {
    V4,
    V6,
}

impl FromSql<Integer, diesel::sqlite::Sqlite> for HistoryIpVersion {
    fn from_sql(bytes: SqliteValue<'_, '_, '_>) -> diesel::deserialize::Result<Self> {
        match i32::from_sql(bytes)? {
            1 => Ok(Self::V4),
            2 => Ok(Self::V6),
            x => Err(format!("Unrecognized variant {}", x).into()),
        }
    }
}

impl ToSql<Integer, diesel::sqlite::Sqlite> for HistoryIpVersion {
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, diesel::sqlite::Sqlite>,
    ) -> diesel::serialize::Result {
        let v = match self {
            Self::V4 => 1,
            Self::V6 => 2,
        };
        out.set_value(v);
        Ok(IsNull::No)
    }
}

#[derive(Serialize)]
pub struct HistoryRes {
    total: i64,
    histories: Vec<History>,
}

impl HistoryRes {
    pub fn new(total: i64, histories: Vec<History>) -> Self {
        Self { total, histories }
    }
}
