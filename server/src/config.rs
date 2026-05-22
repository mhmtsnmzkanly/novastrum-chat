use std::{env, net::SocketAddr};

use crate::error::{AppError, AppResult};

const DEFAULT_APP_ENV: &str = "development";
const DEFAULT_SERVER_HOST: &str = "127.0.0.1";
const DEFAULT_SERVER_PORT: u16 = 8080;
const DATABASE_URL_ENV: &str = "DATABASE_URL";

#[derive(Clone, Debug, serde::Serialize)]
pub struct Config {
    pub app_env: String,
    pub server_host: String,
    pub server_port: u16,
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

        Ok(Self {
            app_env,
            server_host,
            server_port,
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
