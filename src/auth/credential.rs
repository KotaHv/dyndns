use argon2::{
    Argon2,
    password_hash::{
        PasswordHashString, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng,
    },
};

pub struct Credential {
    username: String,
    password_hash: PasswordHashString,
}

impl Credential {
    pub fn new(username: String, password: &str) -> Result<Self, String> {
        let salt = SaltString::generate(&mut OsRng);
        let password_hash_string = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|err| format!("failed to hash auth password: {err}"))?
            .to_string();
        let password_hash = PasswordHashString::new(&password_hash_string)
            .map_err(|err| format!("invalid generated auth password hash: {err}"))?;

        Ok(Self {
            username,
            password_hash,
        })
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn verify(&self, username: &str, password: &str) -> bool {
        if username != self.username {
            return false;
        }
        let parsed_hash = self.password_hash.password_hash();
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok()
    }
}
