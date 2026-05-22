use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationMode {
    Open,
    ApprovalRequired,
    InviteOnly,
}

impl RegistrationMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::ApprovalRequired => "approval_required",
            Self::InviteOnly => "invite_only",
        }
    }
}

impl FromStr for RegistrationMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "open" => Ok(Self::Open),
            "approval_required" => Ok(Self::ApprovalRequired),
            "invite_only" => Ok(Self::InviteOnly),
            other => Err(format!(
                "REGISTRATION_MODE must be one of open, approval_required, invite_only; got `{other}`"
            )),
        }
    }
}

impl fmt::Display for RegistrationMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
