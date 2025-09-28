use std::{error::Error, fs::create_dir_all, path::Path};

use diesel::Connection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::CONFIG;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn run_migrations() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    // This will run the necessary migrations.
    //
    // See the documentation for `MigrationHarness` for
    // all available methods.
    let path = Path::new(&CONFIG.database_url);
    if let Some(path) = path.parent() {
        if !path.exists() {
            create_dir_all(path)?;
        }
    }
    let mut connection = diesel::sqlite::SqliteConnection::establish(&CONFIG.database_url)?;
    connection.run_pending_migrations(MIGRATIONS)?;

    Ok(())
}
