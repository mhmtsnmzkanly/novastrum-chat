use std::collections::HashSet;

use uuid::Uuid;

use crate::db::Database;

/// Permission keys from the specification.
pub const DELETE_MESSAGE: &str = "delete_message";
/// Permission key for account suspension actions.
pub const SUSPEND_USER: &str = "suspend_user";
/// Permission key for discussion edit approvals.
pub const APPROVE_DISCUSSION_EDIT: &str = "approve_discussion_edit";
/// Permission key for basic file uploads.
pub const UPLOAD_FILE_LEVEL_1: &str = "upload_file_level_1";
/// Permission key for medium file uploads.
pub const UPLOAD_FILE_LEVEL_2: &str = "upload_file_level_2";
/// Permission key for unlimited file uploads.
pub const UPLOAD_FILE_LEVEL_3: &str = "upload_file_level_3";
/// Permission key for group moderation.
pub const MODERATE_GROUP: &str = "moderate_group";
/// Permission key for community moderation.
pub const MODERATE_COMMUNITY: &str = "moderate_community";

/// Stateless permission service that reads/writes the permission table.
#[derive(Clone, Debug, Default)]
pub struct PermissionService;

impl PermissionService {
    /// Creates a permission service.
    pub fn new() -> Self {
        Self
    }

    /// Returns default permission set used by bootstrap and new moderation profiles.
    pub fn default_permission_set() -> HashSet<String> {
        [
            UPLOAD_FILE_LEVEL_1,
            MODERATE_GROUP,
            MODERATE_COMMUNITY,
            DELETE_MESSAGE,
            SUSPEND_USER,
            APPROVE_DISCUSSION_EDIT,
        ]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
    }

    /// Grants a single permission to a user row.
    pub async fn grant(&self, db: &Database, user_id: Uuid, permission: &str) {
        db.write(|state| {
            state
                .user_permissions
                .entry(user_id)
                .or_default()
                .insert(permission.to_string());
        })
        .await;
    }

    /// Checks if user owns a permission.
    pub async fn has(&self, db: &Database, user_id: Uuid, permission: &str) -> bool {
        db.read(|state| {
            state
                .user_permissions
                .get(&user_id)
                .map(|set| set.contains(permission))
                .unwrap_or(false)
        })
        .await
    }

    /// Seeds a high-privilege default set (useful for the first admin account).
    pub async fn seed_default_permissions(&self, db: &Database, user_id: Uuid) {
        let defaults = Self::default_permission_set();
        db.write(|state| {
            state.user_permissions.insert(user_id, defaults);
        })
        .await;
    }

    /// Returns the highest upload level available for this user.
    pub async fn upload_level(&self, db: &Database, user_id: Uuid) -> u8 {
        // Level 3 grants unlimited upload permissions.
        if self.has(db, user_id, UPLOAD_FILE_LEVEL_3).await {
            return 3;
        }

        // Level 2 grants medium-sized uploads.
        if self.has(db, user_id, UPLOAD_FILE_LEVEL_2).await {
            return 2;
        }

        // Level 1 grants small uploads.
        if self.has(db, user_id, UPLOAD_FILE_LEVEL_1).await {
            return 1;
        }

        // No permission means uploads are blocked.
        0
    }
}
