use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User lifecycle status values defined by the specification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum UserStatus {
    /// Newly registered account waiting for admin approval.
    Pending = 1,
    /// Fully enabled account with read+write access.
    Active = 2,
    /// Account can read and login but cannot perform write operations.
    Suspended = 3,
    /// Fully blocked account with no login/read/write.
    Banned = 4,
}

impl UserStatus {
    /// Returns whether the status allows authentication.
    pub fn can_login(self) -> bool {
        matches!(self, Self::Active | Self::Suspended)
    }

    /// Returns whether the status allows read operations.
    pub fn can_read(self) -> bool {
        matches!(self, Self::Active | Self::Suspended)
    }

    /// Returns whether the status allows write operations.
    pub fn can_write(self) -> bool {
        matches!(self, Self::Active)
    }
}

impl TryFrom<u8> for UserStatus {
    type Error = String;

    /// Converts stored integer value into a typed `UserStatus` value.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Pending),
            2 => Ok(Self::Active),
            3 => Ok(Self::Suspended),
            4 => Ok(Self::Banned),
            _ => Err(format!("invalid user status value: {value}")),
        }
    }
}

/// Chat classification values required by the spec.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum ChatType {
    /// One-to-one friend conversation.
    DirectMessage = 1,
    /// Multi-member group conversation.
    Group = 2,
    /// Global community channel.
    Community = 3,
}

impl TryFrom<u8> for ChatType {
    type Error = String;

    /// Converts integer chat type to enum with strict validation.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::DirectMessage),
            2 => Ok(Self::Group),
            3 => Ok(Self::Community),
            _ => Err(format!("invalid chat type value: {value}")),
        }
    }
}

/// Account model that mirrors the primary `users` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRecord {
    /// Stable user identifier used by all relational tables.
    pub id: Uuid,
    /// Immutable login username used for authentication.
    pub login_username: String,
    /// Immutable public username used in UI mentions/profile.
    pub public_username: String,
    /// Argon2id password hash.
    pub password_hash: String,
    /// Current account status.
    pub status: UserStatus,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Authenticated browser/device session model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    /// Stable opaque session id written into a secure cookie.
    pub session_id: String,
    /// Session owner user id.
    pub user_id: Uuid,
    /// Session start timestamp.
    pub created_at: DateTime<Utc>,
    /// Last activity timestamp used for inactivity timeout.
    pub last_activity_at: DateTime<Utc>,
    /// Hard expiration timestamp.
    pub expires_at: DateTime<Utc>,
    /// Optional short label for device/session management.
    pub device_label: Option<String>,
}

/// Top-level chat room entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRoom {
    /// Stable room identifier.
    pub id: Uuid,
    /// Room type (DM / Group / Community).
    pub chat_type: ChatType,
    /// User id that created the room.
    pub created_by: Uuid,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Optional human-friendly label.
    #[serde(default)]
    pub label: Option<String>,
}

/// Membership row used for group visibility (`joined_at` rule).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMember {
    /// Parent chat id.
    pub chat_id: Uuid,
    /// Member user id.
    pub user_id: Uuid,
    /// Join timestamp that gates visible message history.
    pub joined_at: DateTime<Utc>,
}

/// Stored chat message payload used by DM/Group/Community tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Stable message identifier.
    pub id: Uuid,
    /// Parent chat id.
    pub chat_id: Uuid,
    /// Chat type to route into dedicated storage table.
    pub chat_type: ChatType,
    /// Sender user id.
    pub sender_id: Uuid,
    /// Normalized body text.
    pub body: String,
    /// Mentioned user ids extracted from `@username` tokens.
    pub mentions: Vec<Uuid>,
    /// Soft delete marker.
    pub deleted: bool,
    /// Message creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Optional edit timestamp.
    pub edited_at: Option<DateTime<Utc>>,
}

/// Flat discussion record with hierarchy represented by `parent_id` + `depth`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscussionPost {
    /// Stable post id.
    pub id: Uuid,
    /// Parent thread id.
    pub thread_id: Uuid,
    /// Optional parent post id.
    pub parent_id: Option<Uuid>,
    /// Cached depth value to enforce max depth = 3.
    pub depth: u8,
    /// Author user id.
    pub author_id: Uuid,
    /// Post title (top-level posts may use it, replies can keep it empty).
    pub title: String,
    /// Post markdown/body content.
    pub body: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Optional edit timestamp.
    pub edited_at: Option<DateTime<Utc>>,
    /// Cached vote score (optional, filled by list handler).
    #[serde(default)]
    pub score: i64,
}

/// Notification row used by unread counters and real-time pushes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationItem {
    /// Stable notification id.
    pub id: Uuid,
    /// Recipient user id.
    pub user_id: Uuid,
    /// Optional related chat id.
    pub chat_id: Option<Uuid>,
    /// Notification category key.
    pub kind: String,
    /// Human-readable message content.
    pub body: String,
    /// Unread marker for counter management.
    pub unread: bool,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Cursor-based response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPage<T> {
    /// Items returned for current page.
    pub items: Vec<T>,
    /// Cursor string for fetching the next page (optional).
    pub next_cursor: Option<String>,
}

/// Discussion edit request status values.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum DiscussionEditStatus {
    Pending = 1,
    Approved = 2,
    Rejected = 3,
}

impl DiscussionEditStatus {
    pub fn is_pending(self) -> bool {
        matches!(self, Self::Pending)
    }
}

impl TryFrom<u8> for DiscussionEditStatus {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Pending),
            2 => Ok(Self::Approved),
            3 => Ok(Self::Rejected),
            _ => Err(format!("invalid discussion edit status: {value}")),
        }
    }
}

/// Represents a request by a user to edit an existing discussion post.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscussionEditRequest {
    /// Request id.
    pub id: Uuid,
    /// Target post id.
    pub post_id: Uuid,
    /// Requesting author id.
    pub author_id: Uuid,
    /// New title (optional).
    pub new_title: Option<String>,
    /// New body content.
    pub new_body: String,
    /// Current status of the request.
    pub status: DiscussionEditStatus,
    /// When the request was created.
    pub requested_at: DateTime<Utc>,
    /// Resolution timestamp if any.
    pub resolved_at: Option<DateTime<Utc>>,
    /// Admin who resolved the request.
    pub resolved_by: Option<Uuid>,
}

/// Stored file metadata (binary content is persisted on local disk path).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// File id and path namespace seed.
    pub id: Uuid,
    /// File owner user id.
    pub owner_id: Uuid,
    /// MIME type validated before accept.
    pub mime_type: String,
    /// Byte size validated against permission upload levels.
    pub size_bytes: u64,
    /// Relative storage path under `/storage/{year}/{month}/{uuid}`.
    pub storage_path: String,
    /// Whether file is download-only (used for zip restriction).
    pub download_only: bool,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// API envelope used by handlers for predictable JSON responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEnvelope<T>
where
    T: Serialize,
{
    /// Indicates whether request was processed successfully.
    pub ok: bool,
    /// Optional result payload.
    pub data: Option<T>,
    /// Human readable message.
    pub message: String,
}

impl<T> ApiEnvelope<T>
where
    T: Serialize,
{
    /// Builds a success response with payload.
    pub fn success(message: impl Into<String>, data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            message: message.into(),
        }
    }

    /// Builds a success response without payload.
    pub fn success_no_data(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            data: None,
            message: message.into(),
        }
    }

    /// Builds an error response with no payload.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            message: message.into(),
        }
    }
}

/// Normalizes username to lowercase for case-insensitive exact matching.
pub fn normalize_username(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

/// Validates immutable username constraints (3-16 chars, ascii alnum/underscore).
pub fn validate_username(value: &str) -> bool {
    let trimmed = value.trim();
    if !(3..=16).contains(&trimmed.len()) {
        return false;
    }
    trimmed
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

/// Validates password against the strict `a-z0-9` policy from the spec.
pub fn validate_password(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
}
