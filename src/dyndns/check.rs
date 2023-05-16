use async_trait::async_trait;

use super::Error;

pub struct CheckResult<T> {
    pub old: Option<T>,
    pub new: Option<T>,
}

impl<T> Default for CheckResult<T> {
    fn default() -> Self {
        Self {
            old: None,
            new: None,
        }
    }
}

impl<T> CheckResult<T> {
    pub fn is_change(&self) -> bool {
        self.new.is_some()
    }
}

#[async_trait]
pub trait GetIp {
    type Ip;
    async fn get_new_ip(&self) -> Result<Self::Ip, Error>;
    async fn get_old_ip(&self) -> Result<Option<Self::Ip>, Error>;
}

#[async_trait]
pub trait CheckIp<T>: 'static + Send + Sync
where
    T: 'static + Send + Sync,
{
    async fn check_result(&self) -> Result<CheckResult<T>, Error>;
}

pub async fn check<T>(c: impl CheckIp<T>) -> CheckResult<T>
where
    T: 'static + Send + Sync,
{
    tokio::spawn(async move {
        match c.check_result().await {
            Ok(result) => result,
            Err(e) => {
                error!("{}", e);
                CheckResult::<T>::default()
            }
        }
    })
    .await
    .unwrap_or_default()
}
