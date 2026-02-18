use anyhow::Result;
use axum::{Json, extract::State, response::IntoResponse};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Arc;
use tokio::{process::Command, task::JoinHandle, time::Duration};

use crate::{AppState, config::AppConfig, models::ApiEnvelope};

/// Planned backup artifact list for one schedule run.
#[derive(Debug, Clone, Serialize)]
pub struct BackupPlan {
    /// Backup folder path in `dump/YYYY-MM-DD-HH` format.
    pub directory: String,
    /// Table-level SQL dump files.
    pub sql_files: Vec<String>,
    /// Final zip artifact path.
    pub archive_file: String,
    /// Schedule interval in hours.
    pub interval_hours: u64,
}

/// Scheduler that runs `pg_dump` exports and retains plan metadata.
#[derive(Clone)]
pub struct BackupService {
    _task: Arc<JoinHandle<()>>,
}

impl BackupService {
    pub fn new(config: AppConfig) -> Self {
        let schedule_config = config.clone();
        let handle = tokio::spawn(async move {
            loop {
                if let Err(error) = Self::execute_once(&schedule_config).await {
                    tracing::error!(error = ?error, "scheduled backup failed");
                }
                tokio::time::sleep(Duration::from_secs(12 * 3600)).await;
            }
        });

        Self {
            _task: Arc::new(handle),
        }
    }

    pub fn plan(&self) -> BackupPlan {
        Self::build_plan(Utc::now())
    }

    fn build_plan(now: DateTime<Utc>) -> BackupPlan {
        let directory = format!("dump/{}", now.format("%Y-%m-%d-%H"));
        let sql_files = Self::tables()
            .iter()
            .map(|table| format!("{}.sql", table))
            .collect();
        let archive_file = format!("{}/backup.zip", directory);

        BackupPlan {
            directory,
            sql_files,
            archive_file,
            interval_hours: 12,
        }
    }

    async fn execute_once(config: &AppConfig) -> Result<()> {
        let plan = Self::build_plan(Utc::now());
        tokio::fs::create_dir_all(&plan.directory).await?;

        for table in Self::tables() {
            let target = format!("{}/{}.sql", plan.directory, table);
            let output = Command::new("pg_dump")
                .arg("-d")
                .arg(&config.database_url)
                .arg("-t")
                .arg(table)
                .arg("-f")
                .arg(&target)
                .output()
                .await?;

            if !output.status.success() {
                tracing::warn!(
                    table = table,
                    stderr = %String::from_utf8_lossy(&output.stderr),
                    "table backup command failed"
                );
            }
        }

        if Self::zip_available().await {
            let mut args = vec!["-j".to_string(), plan.archive_file.clone()];
            for file in &plan.sql_files {
                args.push(format!("{}/{}", plan.directory, file));
            }
            let output = Command::new("zip").args(&args).output().await?;
            if !output.status.success() {
                tracing::warn!(
                    stderr = %String::from_utf8_lossy(&output.stderr),
                    "zip command failed"
                );
            }
        }

        Ok(())
    }

    async fn zip_available() -> bool {
        Command::new("zip").arg("--help").output().await.is_ok()
    }

    fn tables() -> &'static [&'static str] {
        &[
            "users",
            "chats",
            "events",
            "activity_logs",
            "notifications",
            "sessions",
            "discussion_posts",
            "discussion_votes",
            "files",
        ]
    }
}

pub async fn backup_plan_handler(State(state): State<AppState>) -> impl IntoResponse {
    Json(ApiEnvelope::success(
        "backup plan generated",
        state.backup.plan(),
    ))
    .into_response()
}
