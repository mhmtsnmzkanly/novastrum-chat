use crate::config::Config;

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: Config,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}
