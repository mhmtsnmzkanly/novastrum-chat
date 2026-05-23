use axum::http::{header::COOKIE, HeaderMap};

use crate::{
    app_state::AppState,
    auth::{
        repository::{AuthRepository, AuthRepositoryError},
        service::SESSION_COOKIE_NAME,
        session_token::hash_session_token,
    },
    users::model::{DmPolicy, UserStatus},
};

#[derive(Debug)]
pub struct AuthenticatedUser {
    #[allow(dead_code)]
    pub id: u64,
    pub public_id: String,
    pub user_name: String,
    pub public_name: String,
    pub status: UserStatus,
    pub dm_policy: DmPolicy,
}

#[derive(Debug)]
pub enum AuthError {
    AuthRequired,
    PendingApproval,
    Banned,
    DatabaseUnavailable(String),
    Internal,
}

impl From<AuthRepositoryError> for AuthError {
    fn from(error: AuthRepositoryError) -> Self {
        match error {
            AuthRepositoryError::UserNameTaken => Self::Internal,
            AuthRepositoryError::Database => {
                Self::DatabaseUnavailable("Database operation failed".to_string())
            }
        }
    }
}

pub struct CurrentUserAuth<'a> {
    state: &'a AppState,
}

impl<'a> CurrentUserAuth<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn authenticate_headers(
        &self,
        headers: &HeaderMap,
    ) -> Result<AuthenticatedUser, AuthError> {
        self.authenticate_token(session_token_from_headers(headers).as_deref())
            .await
    }

    pub async fn authenticate_token(
        &self,
        session_token: Option<&str>,
    ) -> Result<AuthenticatedUser, AuthError> {
        let session_token = session_token.ok_or(AuthError::AuthRequired)?;
        let session_hash =
            hash_session_token(session_token).map_err(|_| AuthError::AuthRequired)?;
        let pool = self.state.database.pool().ok_or_else(|| {
            AuthError::DatabaseUnavailable(
                self.state
                    .database
                    .unavailable_reason()
                    .unwrap_or("Database pool is not available")
                    .to_string(),
            )
        })?;

        let Some(user) = AuthRepository::new(pool)
            .find_authenticated_user_by_session_hash(&session_hash)
            .await
            .map_err(AuthError::from)?
        else {
            return Err(AuthError::AuthRequired);
        };

        if let Some(error) = authenticated_user_status_error(user.status) {
            return Err(error);
        }

        Ok(user)
    }
}

pub fn session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(COOKIE)?.to_str().ok()?;

    cookie_header.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        (name == SESSION_COOKIE_NAME && !value.is_empty()).then(|| value.to_string())
    })
}

pub fn authenticated_user_status_error(status: UserStatus) -> Option<AuthError> {
    match status {
        UserStatus::Active | UserStatus::Suspended => None,
        UserStatus::Pending => Some(AuthError::PendingApproval),
        UserStatus::Banned => Some(AuthError::Banned),
        UserStatus::Deleted => Some(AuthError::AuthRequired),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::header::COOKIE;

    use super::*;

    #[test]
    fn extracts_session_token_from_cookie_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            "theme=dark; novastrum_session=session-token; other=value"
                .parse()
                .unwrap(),
        );

        assert_eq!(
            session_token_from_headers(&headers).as_deref(),
            Some("session-token")
        );
    }

    #[test]
    fn missing_cookie_has_no_session_token() {
        let headers = HeaderMap::new();

        assert!(session_token_from_headers(&headers).is_none());
    }

    #[test]
    fn applies_authenticated_user_status_policy() {
        assert!(authenticated_user_status_error(UserStatus::Active).is_none());
        assert!(authenticated_user_status_error(UserStatus::Suspended).is_none());
        assert!(matches!(
            authenticated_user_status_error(UserStatus::Pending),
            Some(AuthError::PendingApproval)
        ));
        assert!(matches!(
            authenticated_user_status_error(UserStatus::Banned),
            Some(AuthError::Banned)
        ));
        assert!(matches!(
            authenticated_user_status_error(UserStatus::Deleted),
            Some(AuthError::AuthRequired)
        ));
    }
}
