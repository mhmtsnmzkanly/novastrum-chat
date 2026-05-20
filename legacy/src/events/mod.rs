use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::{Database, DbEventRow};

/// Numeric event ids following the `u8` contract in the specification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum EventType {
    /// Presence event emitted when a WebSocket session connects.
    UserConnected = 1,
    /// Presence event emitted when a WebSocket session disconnects.
    UserDisconnected = 2,
    /// Chat event emitted for new messages.
    MessageSent = 3,
    /// Chat event emitted for message edits.
    MessageEdited = 4,
    /// Chat event emitted for soft/hard message deletions.
    MessageDeleted = 5,
    /// Discussion event emitted for new threads.
    DiscussionCreated = 6,
    /// Discussion event emitted for replies.
    DiscussionReplied = 7,
    /// Discussion event emitted when vote changes.
    VoteCast = 8,
    /// File event emitted for accepted uploads.
    FileUploaded = 9,
    /// DM event emitted when a friend request/DM request is created.
    FriendRequested = 10,
    /// Friendship event emitted after DM request acceptance.
    FriendAccepted = 11,
    /// Notification event emitted when a notification row is created.
    NotificationCreated = 12,
    /// Discussion edit request created.
    DiscussionEditRequested = 13,
    /// Discussion edit request approved.
    DiscussionEditApproved = 14,
    /// Discussion edit request rejected.
    DiscussionEditRejected = 15,
    /// Admin-level permission change.
    AdminPermissionChanged = 16,
    /// Admin rate config update.
    AdminRateConfigUpdated = 17,
}

impl EventType {
    /// Converts enum to the storage `u8` value.
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

impl TryFrom<u8> for EventType {
    type Error = String;

    /// Converts persisted numeric value into typed enum with reserved protection.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Err("event type 0 is reserved".to_string()),
            1 => Ok(Self::UserConnected),
            2 => Ok(Self::UserDisconnected),
            3 => Ok(Self::MessageSent),
            4 => Ok(Self::MessageEdited),
            5 => Ok(Self::MessageDeleted),
            6 => Ok(Self::DiscussionCreated),
            7 => Ok(Self::DiscussionReplied),
            8 => Ok(Self::VoteCast),
            9 => Ok(Self::FileUploaded),
            10 => Ok(Self::FriendRequested),
            11 => Ok(Self::FriendAccepted),
            12 => Ok(Self::NotificationCreated),
            13 => Ok(Self::DiscussionEditRequested),
            14 => Ok(Self::DiscussionEditApproved),
            15 => Ok(Self::DiscussionEditRejected),
            16 => Ok(Self::AdminPermissionChanged),
            17 => Ok(Self::AdminRateConfigUpdated),
            255 => Err("event type 255 is reserved".to_string()),
            _ => Err(format!("unsupported event type value: {value}")),
        }
    }
}

/// Lightweight event service that stores audit rows and triggers async dispatch.
#[derive(Clone, Debug, Default)]
pub struct EventService;

impl EventService {
    /// Creates a new event service instance.
    pub fn new() -> Self {
        Self
    }

    /// Inserts an event row into the audit table (inside the transaction boundary).
    pub async fn insert_event_row(
        &self,
        db: &Database,
        event_type: EventType,
        payload: Value,
    ) -> DbEventRow {
        // Build the immutable event row first so it can be logged and dispatched.
        let row = DbEventRow {
            id: Uuid::new_v4(),
            event_type: event_type.as_u8(),
            payload,
            created_at: Utc::now(),
        };

        // Persist event to the audit table before transaction commit.
        db.write(|state| {
            state.events.push(row.clone());
        })
        .await;

        row
    }

    /// Dispatches post-commit event handling asynchronously and never rolls back writes.
    pub fn dispatch_after_commit(&self, row: DbEventRow) {
        // Event handler failures only produce logs by design (no transaction rollback).
        tokio::spawn(async move {
            tracing::info!(
                event_id = %row.id,
                event_type = row.event_type,
                "event dispatched asynchronously"
            );
        });
    }
}
