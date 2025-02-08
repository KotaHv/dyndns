use async_trait::async_trait;

use super::Error;

pub trait CheckResultTrait {
    type IpType;
    fn old(&self) -> &Self::IpType;
    fn new(&self) -> &Self::IpType;
    fn is_changed(&self) -> bool;
}

#[async_trait]
pub trait GetIpTrait {
    type NewIp;
    type OldIp;
    async fn get_new_ip(&self) -> Result<Self::NewIp, Error>;
    async fn get_old_ip(&self) -> Result<Option<Self::OldIp>, Error>;
}

#[async_trait]
pub trait CheckIpTrait: 'static + Send + Sync {
    type ResultType: CheckResultTrait + Default + Send + Sync;

    async fn check_result(&self) -> Result<Self::ResultType, Error>;
}

pub async fn check<C>(c: C) -> C::ResultType
where
    C: CheckIpTrait + Send + Sync,
    C::ResultType: Default + Send + Sync,
{
    tokio::spawn(async move {
        match c.check_result().await {
            Ok(result) => result,
            Err(e) => {
                error!("{}", e);
                C::ResultType::default()
            }
        }
    })
    .await
    .unwrap_or_default()
}
