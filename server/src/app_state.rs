use crate::{config::Config, db::Database};

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: Config,
    pub database: Database,
}

impl AppState {
    pub fn new(config: Config, database: Database) -> Self {
        Self { config, database }
    }
}
