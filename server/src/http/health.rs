use axum::{extract::State, Json};
use serde::Serialize;
use sqlx::Executor;

use crate::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    state: &'static str,
    service: &'static str,
    app_env: String,
}

pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        state: "ok",
        service: "novastrum-server",
        app_env: state.config.app_env,
    })
}

#[derive(Debug, Serialize)]
pub struct DatabaseHealthResponse {
    state: &'static str,
    database: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

pub async fn database_health(State(state): State<AppState>) -> Json<DatabaseHealthResponse> {
    let Some(pool) = state.database.pool() else {
        return Json(DatabaseHealthResponse {
            state: "error",
            database: "mariadb",
            message: Some(
                state
                    .database
                    .unavailable_reason()
                    .unwrap_or("Database pool is not available")
                    .to_string(),
            ),
        });
    };

    match pool.execute("SELECT 1").await {
        Ok(_) => Json(DatabaseHealthResponse {
            state: "ok",
            database: "mariadb",
            message: None,
        }),
        Err(_) => {
            tracing::warn!("database health check failed");

            Json(DatabaseHealthResponse {
                state: "error",
                database: "mariadb",
                message: Some("Database ping failed".to_string()),
            })
        }
    }
}
