use axum::{extract::State, Json};
use serde::Serialize;

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
