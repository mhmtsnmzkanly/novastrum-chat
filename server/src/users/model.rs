#![allow(dead_code)]

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Pending,
    Active,
    Suspended,
    Banned,
    Deleted,
}

impl UserStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Banned => "banned",
            Self::Deleted => "deleted",
        }
    }
}

impl std::fmt::Display for UserStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DmPolicy {
    Everyone,
    SharedGroupMembers,
    FriendsOnly,
    None,
}

impl DmPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Everyone => "everyone",
            Self::SharedGroupMembers => "shared_group_members",
            Self::FriendsOnly => "friends_only",
            Self::None => "none",
        }
    }
}

impl std::fmt::Display for DmPolicy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
