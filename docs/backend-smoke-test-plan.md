# Backend Smoke Test Plan

## 1. Purpose

This plan validates the current REST backend chain before adding WebSocket delivery or frontend API integration. It focuses on the implemented auth and private chat REST endpoints, the shared response envelope, session-cookie behavior, MariaDB migrations, membership checks, and basic negative cases.

This is a manual smoke plan. It is not an automated integration test suite yet.

## 2. Prerequisites

Required:

- MariaDB is running locally or in a reachable development environment.
- A local development database exists.
- `DATABASE_URL` points to that local development database.
- `sqlx-cli` is installed with MySQL/MariaDB support.
- SQLx migrations have been applied.
- The Axum server is running.
- `curl` or another HTTP client is available.

Do not run these smoke commands against production data.

## 3. Environment Setup

Example local setup:

```sh
export DATABASE_URL='mysql://novastrum_user:password@127.0.0.1:3306/novastrum'
cd server
sqlx migrate run
cargo run
```

If the server uses default configuration, it listens on:

```text
http://127.0.0.1:8080
```

Examples below assume:

```sh
BASE_URL='http://127.0.0.1:8080'
COOKIE_JAR='/tmp/novastrum.cookies'
```

## 4. Health Checks

Service health:

```sh
curl -sS "$BASE_URL/health"
```

Expected:

- HTTP success status.
- `state` is `ok`.
- `type` is `system.health`.

Database health:

```sh
curl -sS "$BASE_URL/health/db"
```

Expected:

- HTTP success status.
- `state` is `ok`.
- `type` is `system.database_health`.
- `data.database` is `mariadb`.

If `DATABASE_URL` is missing or invalid, `/health` should still work but `/health/db` should return `error.database`.

## 5. Auth Smoke Flow

Clear any old cookie jar:

```sh
rm -f /tmp/novastrum.cookies
```

Register Mehmet:

```sh
curl -sS -X POST "$BASE_URL/api/auth/register" \
  -H 'Content-Type: application/json' \
  -d '{
    "user_name": "mehmet",
    "public_name": "Mehmet",
    "password": "password123"
  }'
```

Expected:

- `state` is `ok`.
- `type` is `auth.register`.
- `data.user.user_name` is `mehmet`.

Register Ayse:

```sh
curl -sS -X POST "$BASE_URL/api/auth/register" \
  -H 'Content-Type: application/json' \
  -d '{
    "user_name": "ayse",
    "public_name": "Ayse",
    "password": "password123"
  }'
```

Expected:

- `state` is `ok`.
- `type` is `auth.register`.
- `data.user.user_name` is `ayse`.

Login Mehmet and store the session cookie:

```sh
curl -sS -c "$COOKIE_JAR" -X POST "$BASE_URL/api/auth/login" \
  -H 'Content-Type: application/json' \
  -d '{
    "user_name": "mehmet",
    "password": "password123"
  }'
```

Expected:

- `state` is `ok`.
- `type` is `auth.login`.
- Cookie jar contains `novastrum_session`.

Fetch current user:

```sh
curl -sS -b "$COOKIE_JAR" "$BASE_URL/api/me"
```

Expected:

- `state` is `ok`.
- `type` is `auth.me`.
- Current user is Mehmet.

Logout:

```sh
curl -sS -b "$COOKIE_JAR" -c "$COOKIE_JAR" -X POST "$BASE_URL/api/auth/logout"
```

Expected:

- `state` is `ok`.
- `type` is `auth.logout`.
- Response clears `novastrum_session`.

Verify current user fails after logout:

```sh
curl -sS -b "$COOKIE_JAR" "$BASE_URL/api/me"
```

Expected:

- `state` is `error`.
- `type` is `error.auth`.
- `data.code` is `auth_required`.

Login Mehmet again before continuing with chat tests:

```sh
curl -sS -c "$COOKIE_JAR" -X POST "$BASE_URL/api/auth/login" \
  -H 'Content-Type: application/json' \
  -d '{
    "user_name": "mehmet",
    "password": "password123"
  }'
```

## 6. DM Policy Setup Note

The current default `dm_policy` is `shared_group_members`. A direct conversation between fresh users may be rejected because group features do not exist yet to prove shared group membership.

For local smoke testing only, either:

- Update the target user's `dm_policy` to `everyone` manually.
- Later, when group features exist, create a shared active group membership before testing direct conversation creation.

Local-only SQL example:

```sql
UPDATE users SET dm_policy = 'everyone' WHERE user_name = 'ayse';
```

Run this only against a local development database.

## 7. Direct Conversation Smoke Flow

Create or return Mehmet's direct conversation with Ayse:

```sh
curl -sS -b "$COOKIE_JAR" -X POST "$BASE_URL/api/conversations/direct" \
  -H 'Content-Type: application/json' \
  -d '{
    "target_user_name": "ayse"
  }'
```

Expected:

- `state` is `ok`.
- `type` is `chat.conversation.direct`.
- `data.conversation.public_id` starts with `cnv_`.
- `data.conversation.kind` is `direct`.
- `data.conversation.target_user.user_name` is `ayse`.

Save `data.conversation.public_id` as `CONVERSATION_ID` for later requests.

Example:

```sh
CONVERSATION_ID='cnv_...'
```

## 8. Message Smoke Flow

Send a message:

```sh
curl -sS -b "$COOKIE_JAR" -X POST "$BASE_URL/api/conversations/$CONVERSATION_ID/messages" \
  -H 'Content-Type: application/json' \
  -d '{
    "body": "Merhaba"
  }'
```

Expected:

- `state` is `ok`.
- `type` is `chat.message.created`.
- `data.message.public_id` starts with `msg_`.
- `data.message.conversation_id` matches `CONVERSATION_ID`.
- `data.message.body` is `Merhaba`.

List messages:

```sh
curl -sS -b "$COOKIE_JAR" "$BASE_URL/api/conversations/$CONVERSATION_ID/messages"
```

Expected:

- `state` is `ok`.
- `type` is `chat.messages.list`.
- `data.items` contains the sent message.
- `data.page.has_more` is a boolean.
- `data.page.next_cursor` is either a message public id or `null`.

List conversations:

```sh
curl -sS -b "$COOKIE_JAR" "$BASE_URL/api/conversations"
```

Expected:

- `state` is `ok`.
- `type` is `chat.conversations.list`.
- `data.items` contains the direct conversation.
- `latest_message.body` is `Merhaba`.

## 9. Negative Tests

Invalid registration `user_name`:

```sh
curl -sS -X POST "$BASE_URL/api/auth/register" \
  -H 'Content-Type: application/json' \
  -d '{
    "user_name": "bad-name",
    "public_name": "Bad Name",
    "password": "password123"
  }'
```

Expected: `error.validation` with `validation_failed`.

Wrong password:

```sh
curl -sS -X POST "$BASE_URL/api/auth/login" \
  -H 'Content-Type: application/json' \
  -d '{
    "user_name": "mehmet",
    "password": "wrong-password"
  }'
```

Expected: `error.auth` with `invalid_credentials`.

Current user without cookie:

```sh
curl -sS "$BASE_URL/api/me"
```

Expected: `error.auth` with `auth_required`.

Create direct conversation with self:

```sh
curl -sS -b "$COOKIE_JAR" -X POST "$BASE_URL/api/conversations/direct" \
  -H 'Content-Type: application/json' \
  -d '{
    "target_user_name": "mehmet"
  }'
```

Expected: `error.validation` with `cannot_message_self`.

Create direct conversation with missing user:

```sh
curl -sS -b "$COOKIE_JAR" -X POST "$BASE_URL/api/conversations/direct" \
  -H 'Content-Type: application/json' \
  -d '{
    "target_user_name": "missinguser"
  }'
```

Expected: `error.not_found` with `user_not_found`.

Send empty message:

```sh
curl -sS -b "$COOKIE_JAR" -X POST "$BASE_URL/api/conversations/$CONVERSATION_ID/messages" \
  -H 'Content-Type: application/json' \
  -d '{
    "body": "   "
  }'
```

Expected: `error.validation` with `validation_failed`.

Send message without membership:

```sh
curl -sS -b "$COOKIE_JAR" -X POST "$BASE_URL/api/conversations/cnv_missing/messages" \
  -H 'Content-Type: application/json' \
  -d '{
    "body": "No access"
  }'
```

Expected: `error.not_found` with `conversation_not_found`, or `error.forbidden` with `not_conversation_member` if using a real conversation where the user is not a member.

List messages without membership:

```sh
curl -sS -b "$COOKIE_JAR" "$BASE_URL/api/conversations/cnv_missing/messages"
```

Expected: `error.not_found` with `conversation_not_found`, or `error.forbidden` with `not_conversation_member` if using a real conversation where the user is not a member.

## 10. Expected Response Envelopes

OK response:

```json
{
  "state": "ok",
  "type": "chat.message.created",
  "req": null,
  "data": {}
}
```

Validation error:

```json
{
  "state": "error",
  "type": "error.validation",
  "req": null,
  "data": {
    "code": "validation_failed",
    "message": "Validation failed",
    "fields": {
      "body": "required"
    }
  }
}
```

Authentication required:

```json
{
  "state": "error",
  "type": "error.auth",
  "req": null,
  "data": {
    "code": "auth_required",
    "message": "Authentication is required"
  }
}
```

DM not allowed:

```json
{
  "state": "error",
  "type": "error.forbidden",
  "req": null,
  "data": {
    "code": "dm_not_allowed",
    "message": "Direct messages are not allowed by this user"
  }
}
```

Database unavailable:

```json
{
  "state": "error",
  "type": "error.database",
  "req": null,
  "data": {
    "code": "database_unavailable",
    "message": "DATABASE_URL is not configured"
  }
}
```

## 11. Cleanup

Optional local development cleanup:

```sql
DELETE FROM sessions;
DELETE FROM messages;
DELETE FROM direct_conversation_pairs;
DELETE FROM conversation_memberships;
DELETE FROM conversations;
DELETE FROM users;
```

This cleanup is for local development only. Do not run it against production or shared staging data.

## 12. Known Limitations

Current limitations:

- No WebSocket implementation.
- No frontend API client.
- No unread counts.
- No groups API.
- No discussion API.
- No media upload.
- No automated integration tests yet.
- Conversation list cursor pagination is deferred.
- Direct conversation testing may require manual local `dm_policy` adjustment until group features exist.

## 13. Recommended Next Step

Recommended next step:

- Run this manual REST smoke plan against a local migrated MariaDB database.
- If it passes, create a small manual smoke script that performs the same sequence repeatably.
- After the REST chain is stable, implement the WebSocket skeleton and packet registry wiring.
