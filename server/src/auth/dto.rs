use serde::{Deserialize, Serialize};

use crate::users::model::{DmPolicy, UserStatus};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub user_name: String,
    pub public_name: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub user_name: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub user: PublicUserResponse,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub user: PublicUserResponse,
}

#[derive(Debug, Serialize)]
pub struct CurrentUserResponse {
    pub user: PublicUserResponse,
}

#[derive(Debug, Serialize)]
pub struct PublicUserResponse {
    pub public_id: String,
    pub user_name: String,
    pub public_name: String,
    pub status: UserStatus,
    pub dm_policy: DmPolicy,
}
