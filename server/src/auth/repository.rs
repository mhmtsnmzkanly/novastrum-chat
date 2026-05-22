use sqlx::MySqlPool;

use crate::{auth::service::NewUser, users::model::DmPolicy};

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
