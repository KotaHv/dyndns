use crate::Error;

pub mod ipv4;
pub mod ipv6;

#[derive(Debug)]
pub struct CheckResult<P, C, E> {
    pub previous: P,
    pub current: C,
    pub external: E,
}

impl<P, C, E> Default for CheckResult<P, C, E>
where
    P: Default,
    C: Default,
    E: Default,
{
    fn default() -> Self {
        Self {
            previous: P::default(),
            current: C::default(),
            external: E::default(),
        }
    }
}

impl<P, C, E> CheckResult<P, C, E> {
    pub fn new(previous: P, current: C, external: E) -> Self {
        Self {
            previous,
            current,
            external,
        }
    }
}

pub trait IpChecker: Send + Sync + Sized {
    type Previous: Default + Send + Sync;
    type Current: Default + Send + Sync;
    type External: Default + Send + Sync;

    async fn check(
        self,
    ) -> Result<CheckResult<Self::Previous, Self::Current, Self::External>, Error>;
}

pub async fn run_checker<C>(checker: C) -> CheckResult<C::Previous, C::Current, C::External>
where
    C: IpChecker,
{
    match checker.check().await {
        Ok(result) => result,
        Err(err) => {
            error!("{}", err);
            CheckResult::default()
        }
    }
}
