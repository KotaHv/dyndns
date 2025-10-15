use axum::http::StatusCode;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, errors::ErrorKind};
use serde::{Deserialize, Serialize};

use crate::Error;

pub struct AccessTokenService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    ttl: Duration,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub iat: i64,
}

impl AccessTokenService {
    pub fn new(secret: &str, ttl: Duration) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            ttl,
        }
    }

    pub fn generate(
        &self,
        subject: &str,
        now: DateTime<Utc>,
    ) -> Result<(String, DateTime<Utc>), Error> {
        let expires_at = now + self.ttl;
        let claims = Claims {
            sub: subject.to_string(),
            exp: expires_at.timestamp(),
            iat: now.timestamp(),
        };
        let header = Header::new(Algorithm::HS256);
        let token = jsonwebtoken::encode(&header, &claims, &self.encoding_key).map_err(|err| {
            Error::Custom {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                reason: format!("failed to encode auth token: {err}"),
                code: Some("token_encoding_failed"),
            }
        })?;

        Ok((token, expires_at))
    }

    pub fn verify_access_token(&self, token: &str) -> Result<Claims, Error> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = 0;
        let token_data = jsonwebtoken::decode::<Claims>(token, &self.decoding_key, &validation)
            .map_err(|err| match err.kind() {
                ErrorKind::ExpiredSignature => {
                    Error::unauthorized("token expired", "token_expired")
                }
                _ => Error::unauthorized("invalid token", "invalid_token"),
            })?;
        Ok(token_data.claims)
    }
}
