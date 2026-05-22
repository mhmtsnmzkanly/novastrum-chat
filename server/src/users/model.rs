#![allow(dead_code)]

use std::str::FromStr;

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

impl FromStr for UserStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            "banned" => Ok(Self::Banned),
            "deleted" => Ok(Self::Deleted),
            _ => Err(format!("unsupported user status `{value}`")),
        }
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

impl FromStr for DmPolicy {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "everyone" => Ok(Self::Everyone),
            "shared_group_members" => Ok(Self::SharedGroupMembers),
            "friends_only" => Ok(Self::FriendsOnly),
            "none" => Ok(Self::None),
            _ => Err(format!("unsupported DM policy `{value}`")),
        }
    }
}
