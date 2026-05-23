CREATE TABLE conversations (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    public_id VARCHAR(32) NOT NULL,
    kind VARCHAR(32) NOT NULL,
    title VARCHAR(120) NULL,
    created_by BIGINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL,
    updated_at DATETIME(6) NOT NULL,
    deleted_at DATETIME(6) NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_conversations_public_id (public_id),
    KEY idx_conversations_kind (kind),
    KEY idx_conversations_created_by (created_by),
    CONSTRAINT chk_conversations_kind CHECK (
        kind IN ('direct', 'group')
    ),
    CONSTRAINT fk_conversations_created_by
        FOREIGN KEY (created_by)
        REFERENCES users (id)
        ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE conversation_memberships (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    public_id VARCHAR(32) NOT NULL,
    conversation_id BIGINT UNSIGNED NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    role VARCHAR(32) NOT NULL,
    status VARCHAR(32) NOT NULL,
    -- A value of 0 means no prior message boundary. History queries must use
    -- messages.id > visible_from_message_id for the active membership row.
    -- This cannot be a foreign key while 0 is the sentinel value.
    visible_from_message_id BIGINT UNSIGNED NOT NULL DEFAULT 0,
    joined_at DATETIME(6) NOT NULL,
    left_at DATETIME(6) NULL,
    removed_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_conversation_memberships_public_id (public_id),
    KEY idx_conversation_memberships_conversation_id (conversation_id),
    KEY idx_conversation_memberships_user_id (user_id),
    KEY idx_conversation_memberships_conversation_user (conversation_id, user_id),
    KEY idx_conversation_memberships_conversation_status (conversation_id, status),
    CONSTRAINT chk_conversation_memberships_role CHECK (
        role IN ('owner', 'member')
    ),
    CONSTRAINT chk_conversation_memberships_status CHECK (
        status IN ('active', 'left', 'removed')
    ),
    CONSTRAINT fk_conversation_memberships_conversation_id
        FOREIGN KEY (conversation_id)
        REFERENCES conversations (id)
        ON DELETE CASCADE,
    CONSTRAINT fk_conversation_memberships_user_id
        FOREIGN KEY (user_id)
        REFERENCES users (id)
        ON DELETE RESTRICT,
    CONSTRAINT fk_conversation_memberships_removed_by
        FOREIGN KEY (removed_by)
        REFERENCES users (id)
        ON DELETE SET NULL
    -- MariaDB does not provide simple partial unique indexes. The service layer
    -- must enforce at most one active membership per conversation/user pair.
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE direct_conversation_pairs (
    conversation_id BIGINT UNSIGNED NOT NULL,
    user_low_id BIGINT UNSIGNED NOT NULL,
    user_high_id BIGINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL,
    PRIMARY KEY (conversation_id),
    UNIQUE KEY uq_direct_conversation_pairs_users (user_low_id, user_high_id),
    KEY idx_direct_conversation_pairs_user_high_id (user_high_id),
    CONSTRAINT chk_direct_conversation_pairs_user_order CHECK (
        user_low_id < user_high_id
    ),
    CONSTRAINT fk_direct_conversation_pairs_conversation_id
        FOREIGN KEY (conversation_id)
        REFERENCES conversations (id)
        ON DELETE CASCADE,
    CONSTRAINT fk_direct_conversation_pairs_user_low_id
        FOREIGN KEY (user_low_id)
        REFERENCES users (id)
        ON DELETE RESTRICT,
    CONSTRAINT fk_direct_conversation_pairs_user_high_id
        FOREIGN KEY (user_high_id)
        REFERENCES users (id)
        ON DELETE RESTRICT
    -- Services must canonicalize direct pairs so user_low_id is the lower
    -- internal user id and user_high_id is the higher internal user id.
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE messages (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    public_id VARCHAR(32) NOT NULL,
    conversation_id BIGINT UNSIGNED NOT NULL,
    sender_id BIGINT UNSIGNED NOT NULL,
    body TEXT NULL,
    message_type VARCHAR(32) NOT NULL DEFAULT 'text',
    created_at DATETIME(6) NOT NULL,
    edited_at DATETIME(6) NULL,
    deleted_at DATETIME(6) NULL,
    deleted_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_messages_public_id (public_id),
    KEY idx_messages_conversation_id_id (conversation_id, id),
    KEY idx_messages_sender_id (sender_id),
    KEY idx_messages_deleted_at (deleted_at),
    CONSTRAINT chk_messages_message_type CHECK (
        message_type IN ('text', 'system')
    ),
    CONSTRAINT fk_messages_conversation_id
        FOREIGN KEY (conversation_id)
        REFERENCES conversations (id)
        ON DELETE CASCADE,
    CONSTRAINT fk_messages_sender_id
        FOREIGN KEY (sender_id)
        REFERENCES users (id)
        ON DELETE RESTRICT,
    CONSTRAINT fk_messages_deleted_by
        FOREIGN KEY (deleted_by)
        REFERENCES users (id)
        ON DELETE SET NULL
    -- Deleted messages remain as rows. User deletes should redact body by
    -- setting body = NULL with deleted_at/deleted_by populated.
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
