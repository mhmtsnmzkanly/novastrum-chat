use axum::{extract::State, Json};
use serde::Serialize;
use sqlx::Executor;

use crate::{
    app_state::AppState,
    http::response::{ApiErrorPayload, ApiResponse},
};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    service: &'static str,
    app_env: String,
}

pub async fn health(State(state): State<AppState>) -> Json<ApiResponse<HealthResponse>> {
    Json(ApiResponse::ok(
        "system.health",
        HealthResponse {
            service: "novastrum-server",
            app_env: state.config.app_env,
        },
    ))
}

#[derive(Debug, Serialize)]
pub struct DatabaseHealthResponse {
    database: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum DatabaseHealthData {
    Ok(DatabaseHealthResponse),
    Error(ApiErrorPayload),
}

pub async fn database_health(
    State(state): State<AppState>,
) -> Json<ApiResponse<DatabaseHealthData>> {
    let Some(pool) = state.database.pool() else {
        let message = state
            .database
            .unavailable_reason()
            .unwrap_or("Database pool is not available");

        return Json(ApiResponse::error(
            "error.database",
            DatabaseHealthData::Error(ApiErrorPayload::new("database_unavailable", message)),
        ));
    };

    match pool.execute("SELECT 1").await {
        Ok(_) => Json(ApiResponse::ok(
            "system.database_health",
            DatabaseHealthData::Ok(DatabaseHealthResponse {
                database: "mariadb",
            }),
        )),
        Err(_) => {
            tracing::warn!("database health check failed");

            Json(ApiResponse::error(
                "error.database",
                DatabaseHealthData::Error(ApiErrorPayload::new(
                    "database_unavailable",
                    "Database ping failed",
                )),
            ))
        }
    }
}
