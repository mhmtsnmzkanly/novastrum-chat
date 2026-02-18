use chrono::Utc;
use serde_json::Value;
use tracing_subscriber::{EnvFilter, fmt};
use uuid::Uuid;

use crate::db::{ActivityLogRow, Database};

/// Initializes structured JSON logging for API/runtime observability.
pub fn init_logging() {
    // Respect `RUST_LOG` while keeping a safe default for local runs.
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Emit JSON logs so production log pipelines can parse fields reliably.
    let subscriber = fmt()
        .with_env_filter(env_filter)
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .finish();

    // Ignore duplicate initialization so tests/bench runs do not crash.
    let _ = tracing::subscriber::set_global_default(subscriber);
}

/// Persists an activity log row in the monthly partition view.
pub async fn log_activity(db: &Database, action: &str, user_id: Option<Uuid>, details: Value) {
    // Generate partition key like `2026-02` to support monthly partitioning.
    let now = Utc::now();
    let partition_month = now.format("%Y-%m").to_string();

    // Build immutable row before DB write.
    let row = ActivityLogRow {
        id: Uuid::new_v4(),
        action: action.to_string(),
        user_id,
        details,
        partition_month,
        created_at: now,
    };

    // Insert into activity log table with no retention trimming.
    db.write(|state| {
        state.activity_logs.push(row);
    })
    .await;
}
