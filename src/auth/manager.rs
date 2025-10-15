use std::time::Duration as StdDuration;

use chrono::{DateTime, Duration, Utc};

use crate::{DbPool, Error, config::Auth as AuthConfig, db::AuthSecretRecord};

use super::{
    Claims,
    credential::Credential,
    token::{AccessTokenService, RefreshToken, RefreshTokenService},
};

pub struct AuthManager {
    credential: Credential,
    access_token_service: AccessTokenService,
    refresh_token_service: RefreshTokenService,
}

pub struct AuthToken {
    pub access_token: String,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_token: String,
    pub refresh_expires_at: DateTime<Utc>,
}

impl AuthManager {
    pub async fn new(config: &AuthConfig, pool: DbPool) -> Result<Self, String> {
        let secret = AuthSecretRecord::load_or_create(&pool)
            .await
            .map_err(|err| err.to_string())?;
        let token_ttl = Duration::from_std(StdDuration::from_secs(config.token_ttl_seconds))
            .map_err(|err| format!("invalid authentication token ttl: {err}"))?;
        let refresh_token_ttl =
            Duration::from_std(StdDuration::from_secs(config.refresh_token_ttl_seconds))
                .map_err(|err| format!("invalid authentication refresh token ttl: {err}"))?;

        let credential = Credential::new(config.username.clone(), &config.password)?;
        let access_token_service = AccessTokenService::new(&secret, token_ttl);
        let refresh_token_service = RefreshTokenService::new(pool, refresh_token_ttl);

        Ok(Self {
            credential,
            access_token_service,
            refresh_token_service,
        })
    }

    pub async fn authenticate(&self, username: &str, password: &str) -> Result<AuthToken, Error> {
        if !self.credential.verify(username, password) {
            return Err(Error::unauthorized(
                "invalid credentials",
                "invalid_credentials",
            ));
        }

        self.generate_auth_token(Utc::now()).await
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<AuthToken, Error> {
        let now = Utc::now();
        self.refresh_token_service
            .rotate(now, refresh_token)
            .await?;
        self.generate_auth_token(now).await
    }

    pub async fn revoke(&self, refresh_token: &str) -> Result<(), Error> {
        let now = Utc::now();
        self.refresh_token_service.revoke(now, refresh_token).await
    }

    pub fn verify_access_token(&self, token: &str) -> Result<Claims, Error> {
        self.access_token_service.verify_access_token(token)
    }

    async fn generate_auth_token(&self, now: DateTime<Utc>) -> Result<AuthToken, Error> {
        let (access_token, access_expires_at) = self
            .access_token_service
            .generate(self.credential.username(), now)?;
        let RefreshToken {
            token: refresh_token,
            expires_at: refresh_expires_at,
        } = self.refresh_token_service.create(now).await?;

        Ok(AuthToken {
            access_token,
            access_expires_at,
            refresh_token,
            refresh_expires_at,
        })
    }
}
