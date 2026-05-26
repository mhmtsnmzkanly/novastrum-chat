use crate::{config::Config, db::Database, ws::hub::WsHub};

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: Config,
    pub database: Database,
    pub ws_hub: WsHub,
}

impl AppState {
    pub fn new(config: Config, database: Database) -> Self {
        Self {
            config,
            database,
            ws_hub: WsHub::new(),
        }
    }
}
