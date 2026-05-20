# MariaDB Database Design

This document defines the target MariaDB database design direction for the clean rebuild. It is design documentation only; it does not create migration files or implementation code.

## 1. Executive Summary

Novastrum should use a relational MariaDB schema with explicit tables, foreign keys, constraints, and indexes. Core product data should not be stored as broad JSON blobs. Internal joins use numeric `BIGINT UNSIGNED AUTO_INCREMENT` ids, while APIs expose stable `public_id` strings such as `usr_...`, `cnv_...`, and `msg_...`.

The database must support private direct messages, private group chats with strict membership and history boundaries, public community discussion, combined discussion feeds, user status enforcement, per-user DM policy, deleted-message placeholders, and future packet/media/notification expansion.

## 2. MariaDB Conventions

Baseline conventions:

- Database: MariaDB.
- Engine: InnoDB.
- Charset: `utf8mb4`.
- Collation: `utf8mb4_unicode_ci` or equivalent.
- Internal primary keys: `BIGINT UNSIGNED AUTO_INCREMENT`.
- External ids: unique `public_id` strings.
- Timestamps: `DATETIME(6)`, stored as UTC.
- Core domain data should be relational, not JSON-heavy.
- Explicit foreign keys are required for core relationships.
- Explicit indexes are required for access checks, history, feeds, and uniqueness.

Enum-like values:

- Prefer short `VARCHAR` values with `CHECK` constraints where MariaDB version support is acceptable, or small lookup/static validation in application code.
- Be careful with MariaDB `ENUM`; it is compact but painful to evolve in migrations.
- Values that will likely evolve, such as status and policy, should be documented and validated consistently.

Timestamp convention:

- All application timestamps are UTC.
- Store microsecond precision with `DATETIME(6)`.
- Avoid relying on database/session local timezone behavior.

## 3. ID Strategy

Each core table should use two ids:

- Internal `id`: `BIGINT UNSIGNED AUTO_INCREMENT`, primary key, used for joins and indexes.
- External `public_id`: unique string exposed through APIs, packets, URLs, and frontend state.

Why APIs expose `public_id`:

- Prevents leaking row counts or insertion order.
- Allows stable prefixed ids by domain.
- Keeps public contracts decoupled from internal database ids.
- Supports safer logging, debugging, and packet payloads.

Recommended public id style:

- Prefixed, ULID-like sortable strings are preferred.
- UUID-like strings are acceptable but less readable and less naturally sortable.
- Prefixes make logs and packets easier to inspect.

Examples:

- `usr_01J...` for users.
- `ses_01J...` for sessions.
- `cnv_01J...` for conversations.
- `grp_01J...` for group-specific records if separate group public ids are needed.
- `msg_01J...` for messages.
- `com_01J...` for communities.
- `pst_01J...` for discussion posts.
- `rpl_01J...` for discussion replies.
- `not_01J...` for notifications.

Recommendation:

- Use `public_id VARCHAR(32)` or `VARCHAR(40)` depending on final id format.
- Add `UNIQUE KEY` on every `public_id`.
- Do not use public ids as foreign keys internally.

## 4. User and Auth Tables

### `users`

Purpose: account identity, status, display identity, DM policy, and login credential reference.

Fields:

- `id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY`
- `public_id VARCHAR(...) NOT NULL UNIQUE`
- `user_name VARCHAR(...) NOT NULL UNIQUE`
- `public_name VARCHAR(...) NOT NULL`
- `password_hash VARCHAR(...) NOT NULL`
- `status VARCHAR(...) NOT NULL`
- `dm_policy VARCHAR(...) NOT NULL`
- `created_at DATETIME(6) NOT NULL`
- `updated_at DATETIME(6) NOT NULL`
- `deleted_at DATETIME(6) NULL`

Allowed `status` values:

- `pending`
- `active`
- `suspended`
- `banned`
- `deleted`

Allowed `dm_policy` values:

- `everyone`
- `shared_group_members`
- `friends_only`
- `none`

Notes:

- `user_name` is the unique handle.
- `public_name` is editable display name.
- `user_name` should be immutable initially unless a later product rule allows change-limited handles.
- `deleted_at` supports hidden/deactivated users.
- Banned/deleted behavior is enforced by services, but status must be indexed enough for admin/user queries.

### `sessions`

Purpose: persistent authenticated sessions.

Fields:

- `id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY`
- `public_id VARCHAR(...) NOT NULL UNIQUE`
- `user_id BIGINT UNSIGNED NOT NULL`
- `session_hash VARCHAR(...) NOT NULL UNIQUE`
- `created_at DATETIME(6) NOT NULL`
- `last_seen_at DATETIME(6) NOT NULL`
- `expires_at DATETIME(6) NOT NULL`
- `revoked_at DATETIME(6) NULL`

Foreign keys:

- `sessions.user_id -> users.id`

Notes:

- Store only a hash of the session token.
- `session_hash` must be indexed unique.
- Auth lookup should check `revoked_at IS NULL`, `expires_at > now`, and user status.

### `registration_invites` later

Reserved for invite-only registration:

- Invite token hash.
- Inviter user id.
- Claimed user id.
- Expiry.
- Claimed timestamp.

Do not include as required for first build unless invite-only becomes an initial registration mode.

## 5. Conversation Tables

### `conversations`

Purpose: direct and group private chat containers.

Fields:

- `id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY`
- `public_id VARCHAR(...) NOT NULL UNIQUE`
- `kind VARCHAR(...) NOT NULL`
- `title VARCHAR(...) NULL`
- `created_by BIGINT UNSIGNED NULL`
- `created_at DATETIME(6) NOT NULL`
- `updated_at DATETIME(6) NOT NULL`
- `deleted_at DATETIME(6) NULL`

Allowed `kind` values:

- `direct`
- `group`

Foreign keys:

- `conversations.created_by -> users.id`

Notes:

- Community discussion is not an open chat conversation.
- `title` is nullable for direct conversations and useful for groups.
- Group max size is enforced through membership service logic and membership count queries, not only a table field.

### `conversation_memberships`

Purpose: participant membership for direct and group conversations, including group lifecycle and history boundary.

Fields:

- `id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY`
- `conversation_id BIGINT UNSIGNED NOT NULL`
- `user_id BIGINT UNSIGNED NOT NULL`
- `role VARCHAR(...) NOT NULL`
- `status VARCHAR(...) NOT NULL`
- `visible_from_message_id BIGINT UNSIGNED NULL`
- `joined_at DATETIME(6) NOT NULL`
- `left_at DATETIME(6) NULL`
- `removed_by BIGINT UNSIGNED NULL`

Allowed `role` values:

- `owner`
- `member`

Allowed `status` values:

- `active`
- `left`
- `removed`

Foreign keys:

- `conversation_memberships.conversation_id -> conversations.id`
- `conversation_memberships.user_id -> users.id`
- `conversation_memberships.visible_from_message_id -> messages.id` (requires migration ordering care)
- `conversation_memberships.removed_by -> users.id`

Why lifecycle rows are preferable:

- Leave/rejoin creates a new visibility boundary.
- Historical membership periods matter for message visibility.
- Overwriting a row loses prior membership state and makes audits/resync ambiguous.
- Re-adding a user should not restore access to messages before the new join point.

Constraint recommendation:

- Allow multiple membership rows over time for the same `(conversation_id, user_id)`.
- Enforce at most one active membership per `(conversation_id, user_id)` using service transaction logic or a generated/partial uniqueness workaround, since MariaDB lacks simple partial unique indexes.

## 6. Direct Conversation Uniqueness

Direct conversations need a stronger uniqueness rule than generic memberships. Without it, two users can accidentally get duplicate direct conversations.

Considered approaches:

- Use only `conversation_memberships`: hard to enforce exactly one direct pair uniquely.
- Store canonical pair columns on `conversations`: awkward because group conversations do not have a pair.
- Separate `direct_conversation_pairs` table: clean, explicit, and easy to constrain.

Recommendation: use a separate `direct_conversation_pairs` table.

### `direct_conversation_pairs`

Fields:

- `id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY`
- `conversation_id BIGINT UNSIGNED NOT NULL UNIQUE`
- `user_low_id BIGINT UNSIGNED NOT NULL`
- `user_high_id BIGINT UNSIGNED NOT NULL`
- `created_at DATETIME(6) NOT NULL`

Constraints:

- `UNIQUE KEY (user_low_id, user_high_id)`
- `CHECK (user_low_id < user_high_id)` if supported.

Foreign keys:

- `conversation_id -> conversations.id`
- `user_low_id -> users.id`
- `user_high_id -> users.id`

Rules:

- Canonicalize the pair by sorting internal user ids.
- Create direct conversation and pair row in the same transaction.
- If unique key conflicts, load the existing conversation.

## 7. Group History Boundary

The product requires that new group members cannot see old group messages. Leave/rejoin creates a new boundary.

Recommended boundary:

- `conversation_memberships.visible_from_message_id`

Assignment:

- When adding a member, set `visible_from_message_id` to the current latest message id in that group, or to an equivalent boundary value.
- History query should return messages with `messages.id > visible_from_message_id` for that membership.
- If the group has no messages yet, `visible_from_message_id` can be `NULL`, meaning the member can see future messages from the beginning of the group.

Why `created_at` alone is less safe:

- Timestamps can collide.
- Application/database time precision mistakes can leak or hide boundary messages.
- Clock assumptions become harder under retries and backfills.
- Message id or sequence ordering is easier to index and reason about.

Alternative:

- A monotonic per-conversation message sequence would be even clearer for boundary filtering, such as `visible_from_sequence`.
- This can be considered in `docs/database-design.md` follow-up or schema finalization.

Leave/rejoin:

- On leave/remove, mark current membership row `left` or `removed`.
- On rejoin, create a new membership row with a new `visible_from_message_id`.
- Do not reactivate old membership rows.

Query rule:

- Resolve the user's active membership row.
- Filter by conversation id.
- Filter messages newer than the membership boundary.
- Exclude or placeholder deleted messages according to deletion rules.

## 8. Message Tables

### `messages`

Purpose: stores direct and group chat messages.

Fields:

- `id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY`
- `public_id VARCHAR(...) NOT NULL UNIQUE`
- `conversation_id BIGINT UNSIGNED NOT NULL`
- `sender_id BIGINT UNSIGNED NOT NULL`
- `body TEXT NULL`
- `body_redacted BOOLEAN NOT NULL DEFAULT FALSE`
- `message_type VARCHAR(...) NOT NULL`
- `created_at DATETIME(6) NOT NULL`
- `edited_at DATETIME(6) NULL`
- `deleted_at DATETIME(6) NULL`
- `deleted_by BIGINT UNSIGNED NULL`

Foreign keys:

- `messages.conversation_id -> conversations.id`
- `messages.sender_id -> users.id`
- `messages.deleted_by -> users.id`

Allowed `message_type` examples:

- `text`
- later `system`
- later `media`

Deleted placeholder behavior:

- Deleted messages remain in history.
- Normal users see placeholder state, not the original body.
- Prefer setting `body = NULL`, `body_redacted = TRUE`, `deleted_at`, and `deleted_by`.
- If moderation retention is required later, store retained content outside normal message visibility, not in `messages.body`.

Indexes:

- `(conversation_id, id)` for cursor history and boundary filters.
- `(sender_id, created_at)` for user/admin queries if needed.

## 9. DM Policy Enforcement

Database support:

- `users.dm_policy` stores the recipient's policy.
- `conversation_memberships` and `direct_conversation_pairs` identify existing direct conversations.
- Group membership joins support `shared_group_members`.

Policy behavior:

- `everyone`: allow any authenticated active sender to create/start direct conversation, subject to rate limits.
- `shared_group_members`: require at least one active group conversation membership shared by sender and recipient.
- `friends_only`: reserved until friendships exist.
- `none`: reject new direct conversation creation and first message, except future admin/system exceptions if defined.

Future friendships table:

### `friendships` later

Possible fields:

- `id`
- `user_low_id`
- `user_high_id`
- `status`
- `created_at`
- `ended_at`

Do not include friendships as required for first build unless `friends_only` must be active initially.

Service note:

- DM policy is enforced in the service transaction before direct conversation creation.
- Existing direct conversation behavior after policy changes remains an open product rule.

## 10. Communities

### `communities`

Purpose: Reddit-like topic spaces for public discussion.

Fields:

- `id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY`
- `public_id VARCHAR(...) NOT NULL UNIQUE`
- `slug VARCHAR(...) NOT NULL UNIQUE`
- `name VARCHAR(...) NOT NULL`
- `description TEXT NULL`
- `visibility VARCHAR(...) NOT NULL`
- `created_by BIGINT UNSIGNED NOT NULL`
- `created_at DATETIME(6) NOT NULL`
- `updated_at DATETIME(6) NOT NULL`
- `deleted_at DATETIME(6) NULL`

Allowed `visibility` values:

- `public`
- later `private`

Foreign keys:

- `communities.created_by -> users.id`

Rules:

- Public discussions are readable without joining.
- Posting/replying requires authentication and write permission.
- Initially only admins create communities.
- Private communities are deferred.

### `community_members` or `community_follows`

Purpose: optional personalization and notifications.

Recommendation:

- Use `community_follows` for initial personalization if needed.
- Do not make it required for read access to public communities.

Possible fields:

- `id`
- `community_id`
- `user_id`
- `created_at`
- `notification_level`

## 11. Discussion Tables

### `discussion_posts`

Purpose: top-level posts belonging to communities.

Fields:

- `id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY`
- `public_id VARCHAR(...) NOT NULL UNIQUE`
- `community_id BIGINT UNSIGNED NOT NULL`
- `author_id BIGINT UNSIGNED NOT NULL`
- `title VARCHAR(...) NOT NULL`
- `body TEXT NOT NULL`
- `status VARCHAR(...) NOT NULL`
- `is_solved BOOLEAN NOT NULL DEFAULT FALSE`
- `score INT NOT NULL DEFAULT 0`
- `reply_count INT UNSIGNED NOT NULL DEFAULT 0`
- `created_at DATETIME(6) NOT NULL`
- `updated_at DATETIME(6) NOT NULL`
- `deleted_at DATETIME(6) NULL`

Foreign keys:

- `discussion_posts.community_id -> communities.id`
- `discussion_posts.author_id -> users.id`

`status` examples:

- `open`
- `locked`
- `deleted`

Solved/open support:

- `is_solved` supports solved/open filtering later.
- `status` supports locked/deleted moderation states.

### `discussion_replies`

Purpose: replies to posts, optionally nested later.

Fields:

- `id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY`
- `public_id VARCHAR(...) NOT NULL UNIQUE`
- `post_id BIGINT UNSIGNED NOT NULL`
- `parent_reply_id BIGINT UNSIGNED NULL`
- `author_id BIGINT UNSIGNED NOT NULL`
- `body TEXT NOT NULL`
- `score INT NOT NULL DEFAULT 0`
- `created_at DATETIME(6) NOT NULL`
- `updated_at DATETIME(6) NOT NULL`
- `deleted_at DATETIME(6) NULL`

Foreign keys:

- `discussion_replies.post_id -> discussion_posts.id`
- `discussion_replies.parent_reply_id -> discussion_replies.id`
- `discussion_replies.author_id -> users.id`

### `discussion_votes`

Recommendation: use a unified vote table for posts and replies.

Why unified:

- One vote service and one uniqueness pattern.
- Easier to add target types if needed.
- Avoids duplicated post/reply vote code.

Fields:

- `id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY`
- `public_id VARCHAR(...) NOT NULL UNIQUE`
- `user_id BIGINT UNSIGNED NOT NULL`
- `target_type VARCHAR(...) NOT NULL`
- `post_id BIGINT UNSIGNED NULL`
- `reply_id BIGINT UNSIGNED NULL`
- `value TINYINT NOT NULL`
- `created_at DATETIME(6) NOT NULL`
- `updated_at DATETIME(6) NOT NULL`

Constraints:

- One of `post_id` or `reply_id` must be non-null.
- `target_type` must match the populated target.
- Unique vote per user per target.

MariaDB uniqueness note:

- MariaDB cannot express filtered unique indexes as simply as PostgreSQL.
- Consider either:
  - unified table with generated `target_key` column, unique `(user_id, target_type, target_key)`, or
  - split `discussion_post_votes` and `discussion_reply_votes`.

Recommendation:

- Use unified table only if generated-column uniqueness is acceptable.
- Otherwise split vote tables for simpler constraints.

Combined feed indexes:

- `discussion_posts(community_id, created_at, id)`
- `discussion_posts(created_at, id)` for public combined newest feed.
- Popularity/sort index later based on final score formula.

## 12. Notifications and Presence

Presence:

- Online presence is primarily in memory via `WsHub`.
- Store `users.last_seen_at` later if last-seen display is needed.
- Do not require a full presence table for first database design unless product needs presence history.

Notifications:

- Notification table can be added after concrete events are defined.
- First likely events: DM allowed/requested if requests return, mention created, group member added, discussion reply.

Possible `notifications` fields later:

- `id`
- `public_id`
- `user_id`
- `kind`
- `source_type`
- `source_id`
- `read_at`
- `created_at`

Recommendation:

- Postpone notification schema until API contract identifies concrete notification events.

## 13. Packet Registry

Options:

- Code-only registry.
- Database table.
- Generated schema file.

Recommendation for first implementation:

- Use a generated/static schema file or code registry checked into the server/web contract layer.
- Do not require a runtime database table for packet registry initially.
- Keep `docs/api-packet-format-notes.md` and future `docs/websocket-protocol.md` as human-readable sources.
- Add a database `packet_registry` table only if runtime introspection, admin display, or compatibility negotiation requires it.

Reason:

- Packet ids are protocol metadata, not core domain data.
- Runtime DB lookup for packet type decoding adds unnecessary coupling.
- Drift risk is better handled by generated code/schema checks than by editable DB rows.

## 14. Media/File Upload Later

Reserve future tables:

### `media_files` later

Possible fields:

- `id`
- `public_id`
- `owner_id`
- `storage_key`
- `original_name`
- `mime_type`
- `size_bytes`
- `scan_status`
- `created_at`
- `deleted_at`

### `message_attachments` later

Possible fields:

- `id`
- `message_id`
- `media_file_id`
- `created_at`

### `discussion_attachments` later

Possible fields:

- `id`
- `post_id`
- `reply_id`
- `media_file_id`
- `created_at`

Media is explicitly later because upload security, scanning, quota, access control, and backup policy are not finalized.

## 15. Index Plan

Users:

- `UNIQUE KEY users_public_id_uq (public_id)`
- `UNIQUE KEY users_user_name_uq (user_name)`
- `KEY users_status_idx (status)`

Sessions:

- `UNIQUE KEY sessions_public_id_uq (public_id)`
- `UNIQUE KEY sessions_session_hash_uq (session_hash)`
- `KEY sessions_user_active_idx (user_id, revoked_at, expires_at)`

Conversations:

- `UNIQUE KEY conversations_public_id_uq (public_id)`
- `KEY conversations_kind_idx (kind)`
- `KEY conversations_created_by_idx (created_by)`

Conversation memberships:

- `KEY memberships_user_status_idx (user_id, status)`
- `KEY memberships_conversation_status_idx (conversation_id, status)`
- `KEY memberships_user_conversation_idx (user_id, conversation_id)`
- `KEY memberships_visible_from_idx (conversation_id, user_id, visible_from_message_id)`

Direct pairs:

- `UNIQUE KEY direct_pairs_conversation_uq (conversation_id)`
- `UNIQUE KEY direct_pairs_users_uq (user_low_id, user_high_id)`

Messages:

- `UNIQUE KEY messages_public_id_uq (public_id)`
- `KEY messages_conversation_id_idx (conversation_id, id)`
- `KEY messages_sender_created_idx (sender_id, created_at)`
- optional `KEY messages_conversation_created_idx (conversation_id, created_at, id)`

Communities:

- `UNIQUE KEY communities_public_id_uq (public_id)`
- `UNIQUE KEY communities_slug_uq (slug)`
- `KEY communities_visibility_idx (visibility)`

Discussion posts:

- `UNIQUE KEY discussion_posts_public_id_uq (public_id)`
- `KEY discussion_posts_community_created_idx (community_id, created_at, id)`
- `KEY discussion_posts_created_idx (created_at, id)`
- later `KEY discussion_posts_score_idx (score, created_at, id)` if popularity sort is used.
- `KEY discussion_posts_solved_idx (is_solved, created_at, id)` if solved/open filters are used.

Discussion replies:

- `UNIQUE KEY discussion_replies_public_id_uq (public_id)`
- `KEY discussion_replies_post_created_idx (post_id, created_at, id)`
- `KEY discussion_replies_parent_idx (parent_reply_id)`

Discussion votes:

- `UNIQUE KEY discussion_votes_public_id_uq (public_id)`
- unique vote per user per target via generated target key or split tables.
- `KEY discussion_votes_user_idx (user_id)`
- `KEY discussion_votes_post_idx (post_id)`
- `KEY discussion_votes_reply_idx (reply_id)`

## 16. Constraint Plan

Required constraints:

- Foreign keys for all core relationships.
- Unique `public_id` on all public domain tables.
- Unique `users.user_name`.
- Unique `communities.slug`.
- Direct pair uniqueness on `(user_low_id, user_high_id)`.
- Unique `sessions.session_hash`.
- One vote per user per target.

Important checks:

- User status in allowed set.
- DM policy in allowed set.
- Conversation kind in `direct`, `group`.
- Membership role in `owner`, `member`.
- Membership status in `active`, `left`, `removed`.
- Community visibility in `public`, later `private`.
- Discussion vote target must be exactly one post or reply.

Service-enforced constraints:

- Max group size 10.
- At most one active membership for a user in a conversation if generated/partial uniqueness is not used.
- Owner cannot create invalid group membership states.
- Rejoin must create a new membership boundary.

## 17. Query Patterns

Login session lookup:

- Hash presented session token.
- Find `sessions` by `session_hash`.
- Join `users`.
- Require `revoked_at IS NULL`, `expires_at > UTC now`, and user status not `banned` or `deleted`.

Current user conversations:

- Find active `conversation_memberships` by `user_id`.
- Join `conversations`.
- Return only non-deleted conversations.
- For groups, membership row carries role and visibility boundary.

Direct conversation lookup/create:

- Sort sender and recipient internal ids into `user_low_id`, `user_high_id`.
- In transaction, check recipient `dm_policy`.
- Try insert `conversations(kind='direct')`.
- Insert `direct_conversation_pairs`.
- If unique conflict on pair, load existing conversation.
- Insert memberships if new conversation.

Group message history with visibility boundary:

- Load active membership row for `(conversation_id, user_id)`.
- Require conversation kind `group`.
- Query `messages` where:
  - `conversation_id = ?`
  - `id > COALESCE(visible_from_message_id, 0)`
  - cursor condition by `id` or created/id pair
  - order by `id`
- Return deleted messages as placeholders.

Insert message:

- Validate session and user write status.
- Validate conversation membership.
- For group, validate active membership.
- Insert message.
- Commit.
- Publish realtime packet after commit.

Delete/redact message:

- Validate sender/admin permission.
- Update `messages`:
  - `body = NULL`
  - `body_redacted = TRUE`
  - `deleted_at = UTC now`
  - `deleted_by = actor id`
- Keep row for history placeholder.
- Publish delete packet after commit.

Combined discussion feed:

- Query `discussion_posts`.
- Join `communities`.
- Filter visible communities, initially `communities.visibility = 'public'`.
- Order by newest initially: `discussion_posts.created_at DESC, discussion_posts.id DESC`.
- Cursor by created/id pair.

Community discussion list:

- Find community by slug/public id.
- Require visibility permits read.
- Query posts by `community_id`.
- Apply sort/filter.

Vote insert/update:

- In transaction, upsert one vote per `(user_id, target)`.
- Update target score/count consistently.
- Handle race conditions with unique key and row-level locking or atomic score adjustment.

## 18. Migration Plan

Proposed migration order:

1. `0001_users_sessions.sql`
   - `users`
   - `sessions`
   - core user status and DM policy constraints

2. `0002_conversations_messages.sql`
   - `conversations`
   - `conversation_memberships`
   - `direct_conversation_pairs`
   - `messages`
   - add `visible_from_message_id` FK after `messages` exists if needed

3. `0003_communities_discussion.sql`
   - `communities`
   - optional `community_follows`
   - `discussion_posts`
   - `discussion_replies`
   - `discussion_votes` or split vote tables

4. `0004_packet_registry_or_static_reference.sql` if needed
   - only if packet registry requires runtime DB visibility
   - otherwise omit and keep registry in schema/code/docs

Later:

- notifications
- media files
- message attachments
- discussion attachments
- moderation/audit tables
- registration invites

## 19. Risks and Edge Cases

Group history leak:

- Boundary filters must apply to every history, search, export, and resync path.

`visible_from_message_id` null/zero handling:

- `NULL` should mean "from start/future beginning"; queries must use consistent `COALESCE`.
- Avoid accidentally treating `NULL` as no filter for rejoined members.

Owner leaves group:

- Product rule is still open. Database can represent owner leaving, but service must decide transfer, block, or archive.

Direct conversation duplicates:

- Must use canonical pair uniqueness and transaction handling.

Deleted user content:

- Decide whether old messages/posts show anonymized user, deleted user placeholder, or historical public name.

Deleted message body retention:

- Product behavior redacts body. Moderation retention, if needed, must be separate and access-controlled.

Vote race conditions:

- Concurrent votes can double-adjust scores unless upsert and score updates are transactional.

MariaDB enum migration pain:

- Avoid hard dependency on `ENUM` for values likely to change.

Timezone mistakes:

- Store and compare UTC `DATETIME(6)`.
- Do not mix server local time with database time.

Compact packet registry drift:

- If compact ids are duplicated or payload order changes, clients break.
- Prefer generated/static registry over database-edited packet rows.

## 20. Recommended Next Document

Recommended next documents:

- `docs/api-contract.md`
- `docs/websocket-protocol.md`
- `docs/build-plan.md`

The API contract should define REST packet-inspired responses, pagination, auth behavior, validation errors, and public ids before migrations are written.
