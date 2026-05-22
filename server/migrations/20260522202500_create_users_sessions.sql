CREATE TABLE users (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    public_id VARCHAR(32) NOT NULL,
    user_name VARCHAR(32) NOT NULL,
    public_name VARCHAR(80) NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    status VARCHAR(32) NOT NULL,
    dm_policy VARCHAR(32) NOT NULL,
    created_at DATETIME(6) NOT NULL,
    updated_at DATETIME(6) NOT NULL,
    deleted_at DATETIME(6) NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_users_public_id (public_id),
    UNIQUE KEY uq_users_user_name (user_name),
    KEY idx_users_status (status),
    CONSTRAINT chk_users_status CHECK (
        status IN ('pending', 'active', 'suspended', 'banned', 'deleted')
    ),
    CONSTRAINT chk_users_dm_policy CHECK (
        dm_policy IN ('everyone', 'shared_group_members', 'friends_only', 'none')
    )
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE sessions (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    public_id VARCHAR(32) NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    session_hash VARCHAR(255) NOT NULL,
    created_at DATETIME(6) NOT NULL,
    last_seen_at DATETIME(6) NOT NULL,
    expires_at DATETIME(6) NOT NULL,
    revoked_at DATETIME(6) NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_sessions_public_id (public_id),
    UNIQUE KEY uq_sessions_session_hash (session_hash),
    KEY idx_sessions_user_id (user_id),
    KEY idx_sessions_expires_at (expires_at),
    -- Sessions are authentication artifacts; if a user row is hard-deleted,
    -- its sessions should be removed with it. Normal account deletion is
    -- expected to use users.deleted_at rather than hard delete.
    CONSTRAINT fk_sessions_user_id
        FOREIGN KEY (user_id)
        REFERENCES users (id)
        ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
