use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{AppState, auth, logging, models::ApiEnvelope};

/// Payload for muting/unmuting community notifications.
#[derive(Debug, Deserialize)]
pub struct MuteCommunityPayload {
    /// User applying mute preference.
    pub user_id: Uuid,
    /// Community chat id.
    pub chat_id: Uuid,
    /// Optional mute duration in minutes (`None` means indefinite).
    pub mute_minutes: Option<i64>,
}

/// Updates community mute settings used by notifications.
pub async fn mute_community_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<MuteCommunityPayload>,
) -> impl IntoResponse {
    if let Err(response) =
        auth::require_authenticated_user_match_id(&state, &headers, payload.user_id).await
    {
        return response;
    }
    // Ensure user exists before applying preferences.
    let user_exists = state
        .db
        .read(|db| db.users.contains_key(&payload.user_id))
        .await;
    if !user_exists {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("user not found")),
        );
    }

    // Build optional mute expiration from requested minutes.
    let mute_until = payload
        .mute_minutes
        .map(|minutes| Utc::now() + Duration::minutes(minutes.max(1)));

    // Persist mute settings in notification service.
    state
        .notifications
        .set_chat_mute(payload.user_id, payload.chat_id, mute_until)
        .await;

    // Log mute update for moderation/audit visibility.
    logging::log_activity(
        &state.db,
        "community_mute_updated",
        Some(payload.user_id),
        json!({
            "chat_id": payload.chat_id,
            "mute_until": mute_until,
        }),
    )
    .await;

    (
        StatusCode::OK,
        Json(ApiEnvelope::success(
            "community mute updated",
            json!({"mute_until": mute_until}),
        )),
    )
}
