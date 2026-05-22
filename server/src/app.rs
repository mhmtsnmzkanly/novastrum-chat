use axum::Router;

use crate::{app_state::AppState, http};

pub fn build_router(state: AppState) -> Router {
    http::routes::router().with_state(state)
}
