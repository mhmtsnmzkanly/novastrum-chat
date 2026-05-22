use sqlx::{mysql::MySqlPoolOptions, MySqlPool};

use crate::config::Config;

#[derive(Clone, Debug)]
pub struct Database {
    pool: Option<MySqlPool>,
    unavailable_reason: Option<&'static str>,
}

impl Database {
    pub fn from_config(config: &Config) -> Self {
        let Some(database_url) = config.database_url.as_deref() else {
            return Self {
                pool: None,
                unavailable_reason: Some("DATABASE_URL is not configured"),
            };
        };

        match MySqlPoolOptions::new()
            .max_connections(5)
            .connect_lazy(database_url)
        {
            Ok(pool) => Self {
                pool: Some(pool),
                unavailable_reason: None,
            },
            Err(_) => {
                tracing::warn!("DATABASE_URL is configured but could not create pool");

                Self {
                    pool: None,
                    unavailable_reason: Some("DATABASE_URL is invalid"),
                }
            }
        }
    }

    pub fn pool(&self) -> Option<&MySqlPool> {
        self.pool.as_ref()
    }

    pub fn unavailable_reason(&self) -> Option<&'static str> {
        self.unavailable_reason
    }
}
