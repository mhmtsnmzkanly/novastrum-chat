use std::collections::BTreeMap;

use crate::{
    app_state::AppState,
    auth::{
        dto::{PublicUserResponse, RegisterRequest, RegisterResponse},
        model::RegistrationMode,
        password::{hash_password, validate_password, PasswordError},
        repository::{AuthRepository, AuthRepositoryError, DEFAULT_DM_POLICY},
    },
    public_id::generate_user_public_id,
    users::model::{DmPolicy, UserStatus},
};

pub struct RegisterService<'a> {
    state: &'a AppState,
}

impl<'a> RegisterService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn register(
        &self,
        request: RegisterRequest,
    ) -> Result<RegisterResponse, RegisterError> {
        let normalized = ValidatedRegisterRequest::from_request(request)?;
        let status = registration_status(self.state.config.registration_mode)?;
        let password_hash =
            hash_password(&normalized.password).map_err(|_| RegisterError::Internal)?;
        let pool = self.state.database.pool().ok_or_else(|| {
            RegisterError::DatabaseUnavailable(
                self.state
                    .database
                    .unavailable_reason()
                    .unwrap_or("Database pool is not available")
                    .to_string(),
            )
        })?;

        let user = NewUser {
            public_id: generate_user_public_id(),
            user_name: normalized.user_name,
            public_name: normalized.public_name,
            password_hash,
            status,
            dm_policy: DEFAULT_DM_POLICY,
        };

        AuthRepository::new(pool)
            .insert_user(&user)
            .await
            .map_err(RegisterError::from)?;

        Ok(RegisterResponse {
            user: PublicUserResponse {
                public_id: user.public_id,
                user_name: user.user_name,
                public_name: user.public_name,
                status: user.status,
                dm_policy: user.dm_policy,
            },
        })
    }
}

#[derive(Debug)]
pub struct NewUser {
    pub public_id: String,
    pub user_name: String,
    pub public_name: String,
    pub password_hash: String,
    pub status: UserStatus,
    pub dm_policy: DmPolicy,
}

#[derive(Debug)]
pub enum RegisterError {
    Validation(BTreeMap<&'static str, String>),
    InviteRequired,
    DatabaseUnavailable(String),
    UserNameTaken,
    Internal,
}

impl From<AuthRepositoryError> for RegisterError {
    fn from(error: AuthRepositoryError) -> Self {
        match error {
            AuthRepositoryError::UserNameTaken => Self::UserNameTaken,
            AuthRepositoryError::Database => {
                Self::DatabaseUnavailable("Database operation failed".to_string())
            }
        }
    }
}

#[derive(Debug)]
struct ValidatedRegisterRequest {
    user_name: String,
    public_name: String,
    password: String,
}

impl ValidatedRegisterRequest {
    fn from_request(request: RegisterRequest) -> Result<Self, RegisterError> {
        let mut fields = BTreeMap::new();
        let user_name = normalize_user_name(&request.user_name);
        let public_name = request.public_name.trim().to_string();

        if user_name.is_empty() {
            fields.insert("user_name", "required".to_string());
        } else if !(3..=32).contains(&user_name.len()) {
            fields.insert("user_name", "must be 3 to 32 characters".to_string());
        } else if !is_valid_user_name(&user_name) {
            fields.insert(
                "user_name",
                "must contain only a-z, 0-9, and underscore".to_string(),
            );
        }

        if public_name.is_empty() {
            fields.insert("public_name", "required".to_string());
        } else if public_name.chars().count() > 80 {
            fields.insert("public_name", "must be 1 to 80 characters".to_string());
        }

        if request.password.is_empty() {
            fields.insert("password", "required".to_string());
        } else if matches!(
            validate_password(&request.password),
            Err(PasswordError::TooShort)
        ) {
            fields.insert("password", "must be at least 8 characters".to_string());
        }

        if !fields.is_empty() {
            return Err(RegisterError::Validation(fields));
        }

        Ok(Self {
            user_name,
            public_name,
            password: request.password,
        })
    }
}

pub fn registration_status(mode: RegistrationMode) -> Result<UserStatus, RegisterError> {
    match mode {
        RegistrationMode::Open => Ok(UserStatus::Active),
        RegistrationMode::ApprovalRequired => Ok(UserStatus::Pending),
        // Invite-only is a product mode, but invitation validation is not implemented yet.
        RegistrationMode::InviteOnly => Err(RegisterError::InviteRequired),
    }
}

pub fn normalize_user_name(user_name: &str) -> String {
    user_name.trim().to_ascii_lowercase()
}

pub fn is_valid_user_name(user_name: &str) -> bool {
    user_name
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_user_name() {
        assert_eq!(normalize_user_name(" Mehmet_01 "), "mehmet_01");
    }

    #[test]
    fn validates_user_name_characters() {
        assert!(is_valid_user_name("mehmet_01"));
        assert!(!is_valid_user_name("mehmet-01"));
        assert!(!is_valid_user_name("mehmet.01"));
    }

    #[test]
    fn rejects_invalid_registration_request_fields() {
        let request = RegisterRequest {
            user_name: "me".to_string(),
            public_name: "".to_string(),
            password: "short".to_string(),
        };

        let Err(RegisterError::Validation(fields)) =
            ValidatedRegisterRequest::from_request(request)
        else {
            panic!("expected validation error");
        };

        assert!(fields.contains_key("user_name"));
        assert!(fields.contains_key("public_name"));
        assert!(fields.contains_key("password"));
    }

    #[test]
    fn selects_registration_status_from_mode() {
        assert_eq!(
            registration_status(RegistrationMode::Open).unwrap(),
            UserStatus::Active
        );
        assert_eq!(
            registration_status(RegistrationMode::ApprovalRequired).unwrap(),
            UserStatus::Pending
        );
        assert!(matches!(
            registration_status(RegistrationMode::InviteOnly),
            Err(RegisterError::InviteRequired)
        ));
    }
}
