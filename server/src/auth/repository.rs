use sqlx::{MySqlPool, Row};

use crate::{
    auth::service::{LoginUser, NewSession, NewUser},
    users::model::{DmPolicy, UserStatus},
};

pub struct AuthRepository<'a> {
    pool: &'a MySqlPool,
}

impl<'a> AuthRepository<'a> {
    pub fn new(pool: &'a MySqlPool) -> Self {
        Self { pool }
    }

    pub async fn insert_user(&self, user: &NewUser) -> Result<(), AuthRepositoryError> {
        let result = sqlx::query(
            r#"
            INSERT INTO users (
                public_id,
                user_name,
                public_name,
                password_hash,
                status,
                dm_policy,
                created_at,
                updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, UTC_TIMESTAMP(6), UTC_TIMESTAMP(6))
            "#,
        )
        .bind(&user.public_id)
        .bind(&user.user_name)
        .bind(&user.public_name)
        .bind(&user.password_hash)
        .bind(user.status.as_str())
        .bind(user.dm_policy.as_str())
        .execute(self.pool)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(error) if is_duplicate_user_name(&error) => Err(AuthRepositoryError::UserNameTaken),
            Err(error) => {
                tracing::warn!(%error, "user insert failed");
                Err(AuthRepositoryError::Database)
            }
        }
    }

    pub async fn find_login_user_by_user_name(
        &self,
        user_name: &str,
    ) -> Result<Option<LoginUser>, AuthRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                public_id,
                user_name,
                public_name,
                password_hash,
                status,
                dm_policy
            FROM users
            WHERE user_name = ?
            LIMIT 1
            "#,
        )
        .bind(user_name)
        .fetch_optional(self.pool)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "user lookup failed");
            AuthRepositoryError::Database
        })?;

        let Some(row) = row else {
            return Ok(None);
        };

        let status = row
            .get::<String, _>("status")
            .parse::<UserStatus>()
            .map_err(|error| {
                tracing::warn!(%error, "user row has invalid status");
                AuthRepositoryError::Database
            })?;
        let dm_policy = row
            .get::<String, _>("dm_policy")
            .parse::<DmPolicy>()
            .map_err(|error| {
                tracing::warn!(%error, "user row has invalid dm_policy");
                AuthRepositoryError::Database
            })?;

        Ok(Some(LoginUser {
            id: row.get("id"),
            public_id: row.get("public_id"),
            user_name: row.get("user_name"),
            public_name: row.get("public_name"),
            password_hash: row.get("password_hash"),
            status,
            dm_policy,
        }))
    }

    pub async fn insert_session(&self, session: &NewSession) -> Result<(), AuthRepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO sessions (
                public_id,
                user_id,
                session_hash,
                created_at,
                last_seen_at,
                expires_at
            )
            VALUES (
                ?,
                ?,
                ?,
                UTC_TIMESTAMP(6),
                UTC_TIMESTAMP(6),
                DATE_ADD(UTC_TIMESTAMP(6), INTERVAL 604800 SECOND)
            )
            "#,
        )
        .bind(&session.public_id)
        .bind(session.user_id)
        .bind(&session.session_hash)
        .execute(self.pool)
        .await
        .map(|_| ())
        .map_err(|error| {
            tracing::warn!(%error, "session insert failed");
            AuthRepositoryError::Database
        })
    }
}

#[derive(Debug)]
pub enum AuthRepositoryError {
    UserNameTaken,
    Database,
}

fn is_duplicate_user_name(error: &sqlx::Error) -> bool {
    let sqlx::Error::Database(database_error) = error else {
        return false;
    };

    let is_duplicate_key = database_error.code().as_deref() == Some("1062");
    let references_user_name = database_error.message().contains("uq_users_user_name");
    is_duplicate_key && references_user_name
}

pub const DEFAULT_DM_POLICY: DmPolicy = DmPolicy::SharedGroupMembers;
