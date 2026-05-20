use std::path::PathBuf;

use axum::{
    Json,
    extract::{Multipart, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::{Datelike, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{
    AppState, auth,
    events::EventType,
    logging,
    models::{ApiEnvelope, FileEntry},
};
use tokio::io::AsyncWriteExt;

/// Payload for upload preflight (metadata validation + storage path generation).
#[derive(Debug, Deserialize)]
pub struct PrepareUploadPayload {
    /// Uploader user id.
    pub user_id: Uuid,
    /// MIME type of file.
    pub mime_type: String,
    /// File size in bytes.
    pub size_bytes: u64,
}

/// Validates upload metadata and returns a storage path.
pub async fn prepare_upload_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PrepareUploadPayload>,
) -> impl IntoResponse {
    if let Err(response) =
        auth::require_authenticated_user_match_id(&state, &headers, payload.user_id).await
    {
        return response;
    }
    // Validate uploader existence.
    let user_exists = state
        .db
        .read(|db| db.users.contains_key(&payload.user_id))
        .await;
    if !user_exists {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "uploader not found",
            )),
        );
    }

    // Validate allowed MIME families.
    let (allowed, download_only) = classify_allowed_mime(&payload.mime_type);
    if !allowed {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "mime type not allowed",
            )),
        );
    }

    // Resolve effective upload level based on permission table.
    let upload_level = state
        .permissions
        .upload_level(&state.db, payload.user_id)
        .await;
    if upload_level == 0 {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "missing upload permission",
            )),
        );
    }

    // Enforce permission-level size limits.
    if !size_allowed(upload_level, payload.size_bytes) {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "file exceeds permission level limit",
            )),
        );
    }

    // Build `/storage/{year}/{month}/{uuid}` path.
    let now = Utc::now();
    let file_id = Uuid::new_v4();
    let relative_path = format!("{}/{:02}/{}", now.year(), now.month(), file_id);

    // Ensure local storage directory exists before returning path.
    let full_dir = PathBuf::from(&state.config.storage_root)
        .join(now.year().to_string())
        .join(format!("{:02}", now.month()));
    if let Err(err) = tokio::fs::create_dir_all(&full_dir).await {
        tracing::error!(error = %err, "failed to create storage directory");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "failed to prepare storage path",
            )),
        );
    }

    // Insert metadata row.
    let row = FileEntry {
        id: file_id,
        owner_id: payload.user_id,
        mime_type: payload.mime_type,
        size_bytes: payload.size_bytes,
        storage_path: relative_path.clone(),
        download_only,
        created_at: now,
    };

    state
        .db
        .write(|db| {
            db.files.insert(row.id, row.clone());
        })
        .await;

    // Insert and dispatch file-uploaded event.
    let event = state
        .events
        .insert_event_row(
            &state.db,
            EventType::FileUploaded,
            json!({
                "file_id": row.id,
                "owner_id": row.owner_id,
                "storage_path": row.storage_path,
                "download_only": row.download_only,
            }),
        )
        .await;
    state.events.dispatch_after_commit(event);

    // Log file preflight acceptance.
    logging::log_activity(
        &state.db,
        "file_upload_prepared",
        Some(payload.user_id),
        json!({"file_id": row.id, "size_bytes": row.size_bytes}),
    )
    .await;

    (
        StatusCode::OK,
        Json(ApiEnvelope::success(
            "upload metadata accepted",
            json!({
                "file_id": row.id,
                "storage_path": row.storage_path,
                "download_only": row.download_only,
            }),
        )),
    )
}

pub async fn upload_file_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> impl IntoResponse {
    if let Err(response) = auth::require_authenticated_session(&state, &headers).await {
        return response;
    }

    let mut user_id = None;
    let mut upload_id = None;
    let mut data: Option<Vec<u8>> = None;

    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                if let Some(name) = field.name() {
                    match name {
                        "user_id" => {
                            if let Ok(text) = field.text().await {
                                user_id = Uuid::parse_str(text.trim()).ok();
                            }
                        }
                        "file_id" => {
                            if let Ok(text) = field.text().await {
                                upload_id = Uuid::parse_str(text.trim()).ok();
                            }
                        }
                        "file" => {
                            if let Ok(bytes) = field.bytes().await {
                                data = Some(bytes.to_vec());
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(None) => break,
            Err(err) => {
                tracing::warn!(error = %err, "multipart parse failed");
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiEnvelope::<serde_json::Value>::error(
                        "failed to parse multipart data",
                    )),
                );
            }
        }
    }

    let (user_id, upload_id, file_bytes) = match (user_id, upload_id, data) {
        (Some(user_id), Some(upload_id), Some(bytes)) => (user_id, upload_id, bytes),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "missing upload metadata or file",
                )),
            );
        }
    };

    let entry = state.db.read(|db| db.files.get(&upload_id).cloned()).await;

    let Some(entry) = entry else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "file metadata not found",
            )),
        );
    };

    if entry.owner_id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "upload belongs to different user",
            )),
        );
    }

    if file_bytes.len() as u64 != entry.size_bytes {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "uploaded size mismatch",
            )),
        );
    }

    // Basic mime sniff guard (best-effort).
    let storage_root = state.config.storage_root.clone();
    let destination = storage_root.join(&entry.storage_path);
    if let Some(parent) = destination.parent() {
        if let Err(err) = tokio::fs::create_dir_all(parent).await {
            tracing::error!(error = %err, "failed to create upload parent dir");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "failed to store file",
                )),
            );
        }
    }

    match tokio::fs::File::create(&destination).await {
        Ok(mut file) => {
            if let Err(err) = file.write_all(&file_bytes).await {
                tracing::error!(error = %err, "failed to write upload body");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiEnvelope::<serde_json::Value>::error(
                        "failed to store file",
                    )),
                );
            }
        }
        Err(err) => {
            tracing::error!(error = %err, "failed to open upload target");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "failed to store file",
                )),
            );
        }
    }

    logging::log_activity(
        &state.db,
        "file_uploaded",
        Some(user_id),
        json!({"file_id": upload_id}),
    )
    .await;

    (
        StatusCode::OK,
        Json(ApiEnvelope::success_no_data("file contents saved")),
    )
}

/// Returns `(is_allowed, download_only)` classification for a MIME type.
fn classify_allowed_mime(mime: &str) -> (bool, bool) {
    if mime == "application/zip" {
        return (true, true);
    }

    if mime.starts_with("image/") || mime.starts_with("video/") || mime.starts_with("audio/") {
        return (true, false);
    }

    (false, false)
}

/// Validates size against permission level limits.
fn size_allowed(level: u8, size_bytes: u64) -> bool {
    match level {
        1 => size_bytes <= 2 * 1024 * 1024,
        2 => size_bytes <= 20 * 1024 * 1024,
        3 => true,
        _ => false,
    }
}
