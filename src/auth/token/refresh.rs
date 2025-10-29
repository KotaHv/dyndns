use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use diesel::result::{DatabaseErrorKind, Error as DieselResultError};
use sha2::{Digest, Sha256};

use crate::{
    DbPool, Error, db::RefreshTokenRecord, error::DatabaseError, util::random_urlsafe_string,
};

use subtle::ConstantTimeEq;

pub struct RefreshTokenService {
    pool: DbPool,
    ttl: Duration,
}

pub struct RefreshToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

impl RefreshTokenService {
    pub fn new(pool: DbPool, ttl: Duration) -> Self {
        Self { pool, ttl }
    }

    pub async fn create(&self, now: DateTime<Utc>) -> Result<RefreshToken, Error> {
        let refresh_expires_at = now + self.ttl;
        let conn = self.pool.get().await?;
        RefreshTokenRecord::delete_expired(&conn, now.naive_utc()).await?;

        let token = loop {
            let payload = RefreshTokenPayload::generate();
            let record = RefreshTokenRecord {
                selector: payload.selector.clone(),
                verifier_hash: payload.verifier_hash.clone(),
                expires_at: refresh_expires_at.naive_utc(),
                created_at: now.naive_utc(),
            };

            match RefreshTokenRecord::insert(&conn, record).await {
                Ok(()) => break payload.token,
                Err(Error::Database(DatabaseError::Diesel(DieselResultError::DatabaseError(
                    DatabaseErrorKind::UniqueViolation,
                    _,
                )))) => continue,
                Err(err) => return Err(err),
            }
        };

        Ok(RefreshToken {
            token,
            expires_at: refresh_expires_at,
        })
    }

    pub async fn rotate(&self, now: DateTime<Utc>, refresh_token: &str) -> Result<(), Error> {
        let (selector, verifier) = split_refresh_token(refresh_token)?;
        let conn = self.pool.get().await?;

        let Some(record) = RefreshTokenRecord::find(&conn, &selector).await? else {
            return Err(Error::unauthorized(
                "invalid refresh token",
                "invalid_refresh_token",
            ));
        };

        if record.expires_at <= now.naive_utc() {
            RefreshTokenRecord::delete(&conn, &selector).await?;
            return Err(Error::unauthorized(
                "refresh token expired",
                "refresh_token_expired",
            ));
        }

        RefreshTokenPayload::verify_hash(&verifier, &record.verifier_hash)?;

        RefreshTokenRecord::delete(&conn, &selector).await?;
        RefreshTokenRecord::delete_expired(&conn, now.naive_utc()).await?;

        Ok(())
    }

    pub async fn revoke(&self, now: DateTime<Utc>, refresh_token: &str) -> Result<(), Error> {
        let (selector, verifier) = split_refresh_token(refresh_token)?;
        let conn = self.pool.get().await?;

        if let Some(record) = RefreshTokenRecord::find(&conn, &selector).await? {
            RefreshTokenPayload::verify_hash(&verifier, &record.verifier_hash)?;
            RefreshTokenRecord::delete(&conn, &selector).await?;
        }

        RefreshTokenRecord::delete_expired(&conn, now.naive_utc()).await?;
        Ok(())
    }
}

struct RefreshTokenPayload {
    token: String,
    selector: String,
    verifier_hash: String,
}

fn split_refresh_token(token: &str) -> Result<(String, String), Error> {
    let mut parts = token.splitn(2, '.');
    let invalid = || Error::unauthorized("invalid refresh token", "invalid_refresh_token");

    let selector = parts.next().ok_or_else(invalid)?;
    let verifier = parts.next().ok_or_else(invalid)?;

    if selector.is_empty() || verifier.is_empty() {
        return Err(invalid());
    }

    Ok((selector.to_string(), verifier.to_string()))
}

impl RefreshTokenPayload {
    fn generate() -> Self {
        let selector = random_urlsafe_string(16);
        let verifier = random_urlsafe_string(32);
        let token = format!("{selector}.{verifier}");
        let verifier_hash = Self::hash_verifier(&verifier);
        Self {
            token,
            selector,
            verifier_hash,
        }
    }

    fn hash_verifier(verifier: &str) -> String {
        let digest = Sha256::digest(verifier.as_bytes());
        URL_SAFE_NO_PAD.encode(digest)
    }

    fn verify_hash(verifier: &str, stored_hash: &str) -> Result<(), Error> {
        if stored_hash
            .as_bytes()
            .ct_eq(Self::hash_verifier(verifier).as_bytes())
            .into()
        {
            Ok(())
        } else {
            Err(Error::unauthorized(
                "invalid refresh token",
                "invalid_refresh_token",
            ))
        }
    }
}
