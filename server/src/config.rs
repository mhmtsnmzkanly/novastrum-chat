use std::{env, net::SocketAddr};

use crate::{
    auth::model::RegistrationMode,
    error::{AppError, AppResult},
};

const DEFAULT_APP_ENV: &str = "development";
const DEFAULT_SERVER_HOST: &str = "127.0.0.1";
const DEFAULT_SERVER_PORT: u16 = 8080;
const DATABASE_URL_ENV: &str = "DATABASE_URL";
const REGISTRATION_MODE_ENV: &str = "REGISTRATION_MODE";

#[derive(Clone, Debug, serde::Serialize)]
pub struct Config {
    pub app_env: String,
    pub server_host: String,
    pub server_port: u16,
    pub registration_mode: RegistrationMode,
    #[serde(skip)]
    pub database_url: Option<String>,
}

impl Config {
    pub fn from_env() -> AppResult<Self> {
        let app_env = env::var("APP_ENV").unwrap_or_else(|_| DEFAULT_APP_ENV.to_string());
        let server_host =
            env::var("SERVER_HOST").unwrap_or_else(|_| DEFAULT_SERVER_HOST.to_string());
        let server_port = match env::var("SERVER_PORT") {
            Ok(value) => value.parse::<u16>().map_err(|_| {
                AppError::config(format!("SERVER_PORT must be a valid u16, got `{value}`"))
            })?,
            Err(_) => DEFAULT_SERVER_PORT,
        };
        let database_url = env::var(DATABASE_URL_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty());
        let registration_mode = match env::var(REGISTRATION_MODE_ENV) {
            Ok(value) => value
                .parse::<RegistrationMode>()
                .map_err(AppError::config)?,
            Err(_) => RegistrationMode::Open,
        };

        Ok(Self {
            app_env,
            server_host,
            server_port,
            registration_mode,
            database_url,
        })
    }

    pub fn bind_addr(&self) -> AppResult<SocketAddr> {
        format!("{}:{}", self.server_host, self.server_port)
            .parse()
            .map_err(|source| {
                AppError::config(format!(
                    "SERVER_HOST/SERVER_PORT do not form a valid socket address: {source}"
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_mode_parses_supported_values() {
        assert_eq!(
            "open".parse::<RegistrationMode>().unwrap(),
            RegistrationMode::Open
        );
        assert_eq!(
            "approval_required".parse::<RegistrationMode>().unwrap(),
            RegistrationMode::ApprovalRequired
        );
        assert_eq!(
            "invite_only".parse::<RegistrationMode>().unwrap(),
            RegistrationMode::InviteOnly
        );
    }

    #[test]
    fn registration_mode_rejects_unknown_values() {
        assert!("closed".parse::<RegistrationMode>().is_err());
    }
}
