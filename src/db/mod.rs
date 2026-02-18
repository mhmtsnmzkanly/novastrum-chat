use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Executor, PgPool, Postgres, Row, postgres::PgPoolOptions};
use tokio::sync::Mutex;
use url::Url;
use uuid::Uuid;

use crate::models::{
    ChatMember, ChatMessage, ChatRoom, DiscussionEditRequest, DiscussionPost, FileEntry,
    SessionRecord, UserRecord,
};

const SNAPSHOT_ROW_ID: i16 = 1;

/// Event audit row persisted inside transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbEventRow {
    /// Event id for traceability.
    pub id: Uuid,
    /// Numeric event type (`u8`, with 0/255 reserved by spec).
    pub event_type: u8,
    /// Arbitrary JSON payload.
    pub payload: Value,
    /// Insert timestamp.
    pub created_at: DateTime<Utc>,
}

/// Activity/system log row with monthly partition key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityLogRow {
    /// Log row id.
    pub id: Uuid,
    /// Action key (e.g. user_login, message_sent).
    pub action: String,
    /// Optional actor user id.
    pub user_id: Option<Uuid>,
    /// Structured JSON details.
    pub details: Value,
    /// Partition marker `YYYY-MM`.
    pub partition_month: String,
    /// Insert timestamp.
    pub created_at: DateTime<Utc>,
}

/// Pending DM request entry for non-friend users.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmRequestRow {
    /// DM request id.
    pub id: Uuid,
    /// Sender user id.
    pub from_user_id: Uuid,
    /// Recipient user id.
    pub to_user_id: Uuid,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Persistent application state payload stored in PostgreSQL.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseState {
    /// Primary user rows indexed by id.
    pub users: HashMap<Uuid, UserRecord>,
    /// Case-insensitive login lookup index.
    pub users_by_login: HashMap<String, Uuid>,
    /// Case-insensitive public username lookup index.
    pub users_by_public: HashMap<String, Uuid>,
    /// Session table keyed by session id.
    pub sessions: HashMap<String, SessionRecord>,
    /// Chat room table keyed by chat id.
    pub chats: HashMap<Uuid, ChatRoom>,
    /// Membership table for group visibility rules.
    pub chat_members: Vec<ChatMember>,
    /// Direct message table.
    pub dm_messages: Vec<ChatMessage>,
    /// Group message table.
    pub group_messages: Vec<ChatMessage>,
    /// Community message table.
    pub community_messages: Vec<ChatMessage>,
    /// Discussion post table.
    pub discussion_posts: HashMap<Uuid, DiscussionPost>,
    /// Vote table (`post_id`, `user_id`) => `-1 | 1`.
    pub discussion_votes: HashMap<(Uuid, Uuid), i8>,
    /// Notification rows kept for unread counters and listing.
    pub notifications: Vec<crate::models::NotificationItem>,
    /// File metadata rows.
    pub files: HashMap<Uuid, FileEntry>,
    /// Audit event table.
    pub events: Vec<DbEventRow>,
    /// Activity/system log table.
    pub activity_logs: Vec<ActivityLogRow>,
    /// Permission table keyed by user id.
    pub user_permissions: HashMap<Uuid, HashSet<String>>,
    /// Friend links represented as normalized pair tuples.
    pub friendships: HashSet<(Uuid, Uuid)>,
    /// Pending DM requests from non-friends.
    pub pending_dm_requests: Vec<DmRequestRow>,
    /// Discussion edit moderation requests.
    pub discussion_edit_requests: Vec<DiscussionEditRequest>,
}

/// Shared database handle backed by PostgreSQL.
#[derive(Clone, Debug)]
pub struct Database {
    /// PostgreSQL connection pool.
    pool: PgPool,
    /// Global write lock to serialize read-modify-write snapshots.
    write_lock: Arc<Mutex<()>>,
}

impl Database {
    /// Connects to PostgreSQL, applies schema, and prepares snapshot row.
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        Self::ensure_database_exists(database_url).await?;

        // Open pool with conservative connection count for single-node deployment.
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .with_context(|| format!("failed to connect postgres at {database_url}"))?;

        let db = Self {
            pool,
            write_lock: Arc::new(Mutex::new(())),
        };

        // Apply schema/migrations before serving requests.
        db.apply_migration_sql().await?;

        // Ensure snapshot row exists so read/write calls always have a payload row.
        db.ensure_snapshot_row().await?;

        Ok(db)
    }

    /// Exposes the raw pool for optional direct SQL calls.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Loads a consistent state snapshot and runs a read closure on it.
    pub async fn read<R>(&self, f: impl FnOnce(&DatabaseState) -> R) -> R {
        // Read path fails fast because database is required source-of-truth.
        let state = self
            .load_state(&self.pool, false)
            .await
            .unwrap_or_else(|error| panic!("failed to read postgres state snapshot: {error}"));

        f(&state)
    }

    /// Runs a write closure in a PostgreSQL transaction and persists updated state.
    pub async fn write<R>(&self, f: impl FnOnce(&mut DatabaseState) -> R) -> R {
        // Serialize writes to prevent lost updates between concurrent handlers.
        let _guard = self.write_lock.lock().await;

        // Open transaction for read-modify-write cycle.
        let mut tx = self
            .pool
            .begin()
            .await
            .unwrap_or_else(|error| panic!("failed to begin postgres tx: {error}"));

        // Lock snapshot row before mutation.
        let mut state = self
            .load_state(&mut *tx, true)
            .await
            .unwrap_or_else(|error| panic!("failed to load snapshot for update: {error}"));

        // Execute caller mutation closure.
        let result = f(&mut state);

        // Persist snapshot update and commit.
        self.store_state(&mut *tx, &state)
            .await
            .unwrap_or_else(|error| panic!("failed to persist snapshot: {error}"));

        tx.commit()
            .await
            .unwrap_or_else(|error| panic!("failed to commit postgres tx: {error}"));

        result
    }

    /// Applies migration SQL file containing baseline schema definitions.
    async fn apply_migration_sql(&self) -> anyhow::Result<()> {
        let sql = include_str!("../../migrations/0001_init.sql");

        sqlx::raw_sql(sql)
            .execute(&self.pool)
            .await
            .context("failed to apply migration SQL")?;

        Ok(())
    }

    /// Ensures the single snapshot row exists.
    async fn ensure_snapshot_row(&self) -> anyhow::Result<()> {
        let payload = serde_json::to_value(DatabaseState::default())
            .context("failed to serialize default snapshot")?;

        sqlx::query(
            "INSERT INTO state_snapshot (id, payload) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING",
        )
        .bind(SNAPSHOT_ROW_ID)
        .bind(payload)
        .execute(&self.pool)
        .await
        .context("failed to ensure state_snapshot row")?;

        Ok(())
    }

    /// Loads state snapshot from database (optionally row-locking for update).
    async fn load_state<'e, E>(
        &self,
        executor: E,
        for_update: bool,
    ) -> anyhow::Result<DatabaseState>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let query = if for_update {
            "SELECT payload FROM state_snapshot WHERE id = $1 FOR UPDATE"
        } else {
            "SELECT payload FROM state_snapshot WHERE id = $1"
        };

        let row = sqlx::query(query)
            .bind(SNAPSHOT_ROW_ID)
            .fetch_optional(executor)
            .await
            .context("failed to load state_snapshot row")?;

        let Some(row) = row else {
            return Ok(DatabaseState::default());
        };

        let payload: Value = row
            .try_get("payload")
            .context("failed to decode state_snapshot payload")?;

        let state: DatabaseState = serde_json::from_value(payload)
            .context("failed to deserialize state_snapshot payload")?;

        Ok(state)
    }

    /// Stores updated state snapshot in database.
    async fn store_state<'e, E>(&self, executor: E, state: &DatabaseState) -> anyhow::Result<()>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let payload = serde_json::to_value(state).context("failed to encode state snapshot")?;

        sqlx::query("UPDATE state_snapshot SET payload = $1, updated_at = NOW() WHERE id = $2")
            .bind(payload)
            .bind(SNAPSHOT_ROW_ID)
            .execute(executor)
            .await
            .context("failed to update state_snapshot row")?;

        Ok(())
    }

    async fn ensure_database_exists(database_url: &str) -> anyhow::Result<()> {
        let url =
            Url::parse(database_url).context("failed to parse DATABASE_URL for auto-creation")?;
        let db_name = url.path().trim_start_matches('/').trim();
        if db_name.is_empty() {
            return Ok(());
        }

        let mut admin_url = url.clone();
        admin_url.set_path("/postgres");
        admin_url.set_query(None);

        let admin_pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(admin_url.as_ref())
            .await
            .context("failed to connect to postgres for DB creation")?;

        let query = format!("CREATE DATABASE \"{db_name}\"");
        match sqlx::query(&query).execute(&admin_pool).await {
            Ok(_) => {
                tracing::info!(database = db_name, "created missing database");
            }
            Err(err) => {
                let should_ignore = err
                    .as_database_error()
                    .and_then(|db_err| db_err.code())
                    .map(|code| code == "42P04")
                    .unwrap_or(false);
                if !should_ignore {
                    return Err(err).context("failed to ensure database exists");
                }
            }
        }

        Ok(())
    }
}

/// Converts two user ids into a normalized friendship key.
pub fn normalize_friend_pair(a: Uuid, b: Uuid) -> (Uuid, Uuid) {
    // The smallest UUID comes first so `(A,B)` and `(B,A)` map to one row.
    if a <= b { (a, b) } else { (b, a) }
}
