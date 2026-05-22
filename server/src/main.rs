mod app;
mod app_state;
mod config;
mod db;
mod error;
mod http;

use app_state::AppState;
use config::Config;
use error::AppResult;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> AppResult<()> {
    init_tracing();

    let config = Config::from_env()?;
    let bind_addr = config.bind_addr()?;
    let database = db::Database::from_config(&config);
    let app_state = AppState::new(config, database);
    let router = app::build_router(app_state);

    tracing::info!(%bind_addr, "starting novastrum server");

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "novastrum_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}
