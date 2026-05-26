#!/usr/bin/env bash
set -u

BASE_URL="${BASE_URL:-http://127.0.0.1:8080}"
SKIP_DM_POLICY_NOTE="${SKIP_DM_POLICY_NOTE:-false}"
PASSWORD='smoke-password-123'
MEHMET_USER='mehmet_smoke'
AYSE_USER='ayse_smoke'
COOKIE_JAR="$(mktemp /tmp/novastrum-smoke-cookies.XXXXXX)"
BODY_FILE="$(mktemp /tmp/novastrum-smoke-body.XXXXXX)"

cleanup() {
  rm -f "$COOKIE_JAR" "$BODY_FILE"
}
trap cleanup EXIT

step() {
  printf '\n==> %s\n' "$1"
}

fail() {
  printf '\nSmoke test failed: %s\n' "$1" >&2
  printf 'Last response body:\n' >&2
  if [ -s "$BODY_FILE" ]; then
    cat "$BODY_FILE" >&2
    printf '\n' >&2
  else
    printf '(empty)\n' >&2
  fi
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'Required command not found: %s\n' "$1" >&2
    exit 1
  fi
}

request() {
  local method="$1"
  local path="$2"
  local data="${3:-}"
  local cookie_mode="${4:-none}"
  local status
  local args=(-sS -o "$BODY_FILE" -w '%{http_code}' -X "$method")

  if [ "$cookie_mode" = 'send' ]; then
    args+=(-b "$COOKIE_JAR")
  elif [ "$cookie_mode" = 'save' ]; then
    args+=(-c "$COOKIE_JAR")
  elif [ "$cookie_mode" = 'send-save' ]; then
    args+=(-b "$COOKIE_JAR" -c "$COOKIE_JAR")
  fi

  if [ -n "$data" ]; then
    args+=(-H 'Content-Type: application/json' -d "$data")
  fi

  status="$(curl "${args[@]}" "$BASE_URL$path")"
  printf 'HTTP %s\n' "$status"
  cat "$BODY_FILE"
  printf '\n'

  if [ "$status" -ge 500 ]; then
    fail "unexpected server error for $method $path"
  fi
}

expect_type() {
  local expected="$1"
  if ! grep -q "\"type\":\"$expected\"" "$BODY_FILE"; then
    fail "expected response type $expected"
  fi
}

expect_code() {
  local expected="$1"
  if ! grep -q "\"code\":\"$expected\"" "$BODY_FILE"; then
    fail "expected error code $expected"
  fi
}

expect_ok_or_existing_user() {
  local user_name="$1"
  if grep -q '"type":"auth.register"' "$BODY_FILE"; then
    printf 'Registered %s.\n' "$user_name"
    return
  fi
  if grep -q '"code":"user_name_taken"' "$BODY_FILE"; then
    printf 'User %s already exists; continuing with login smoke flow.\n' "$user_name"
    return
  fi
  fail "expected auth.register or user_name_taken for $user_name"
}

extract_conversation_id() {
  if command -v jq >/dev/null 2>&1; then
    jq -r '.data.conversation.public_id // empty' "$BODY_FILE"
    return
  fi

  sed -n 's/.*"public_id":"\(cnv_[^"]*\)".*/\1/p' "$BODY_FILE" | head -n 1
}

require_command curl

printf 'Novastrum backend REST smoke test\n'
printf 'BASE_URL=%s\n' "$BASE_URL"
printf 'Cookie jar: %s\n' "$COOKIE_JAR"
printf 'Cleanup SQL is documented in docs/backend-smoke-test-plan.md.\n'
printf 'This script does not delete or mutate data except through the tested API calls.\n'

step '1. GET /health'
request GET '/health'
expect_type 'system.health'

step '2. GET /health/db'
request GET '/health/db'
expect_type 'system.database_health'

step '3. Register Mehmet smoke user'
request POST '/api/auth/register' "{
  \"user_name\": \"$MEHMET_USER\",
  \"public_name\": \"Mehmet Smoke\",
  \"password\": \"$PASSWORD\"
}"
expect_ok_or_existing_user "$MEHMET_USER"

step '4. Register Ayse smoke user'
request POST '/api/auth/register' "{
  \"user_name\": \"$AYSE_USER\",
  \"public_name\": \"Ayse Smoke\",
  \"password\": \"$PASSWORD\"
}"
expect_ok_or_existing_user "$AYSE_USER"

step '5. Login Mehmet and save cookie'
request POST '/api/auth/login' "{
  \"user_name\": \"$MEHMET_USER\",
  \"password\": \"$PASSWORD\"
}" save
expect_type 'auth.login'

step '6. GET /api/me with cookie'
request GET '/api/me' '' send
expect_type 'auth.me'

if [ "$SKIP_DM_POLICY_NOTE" != 'true' ]; then
  step '7. DM policy note'
  printf 'Default dm_policy is shared_group_members. Fresh smoke users may not be able to create a direct conversation.\n'
  printf 'For local smoke testing only, run this SQL if the next step returns dm_not_allowed:\n'
  printf "UPDATE users SET dm_policy = 'everyone' WHERE user_name = '%s';\n" "$AYSE_USER"
fi

step '8. Create direct conversation with Ayse'
request POST '/api/conversations/direct' "{
  \"target_user_name\": \"$AYSE_USER\"
}" send
if grep -q '"code":"dm_not_allowed"' "$BODY_FILE"; then
  printf '\nDirect conversation was blocked by dm_policy.\n' >&2
  printf 'For local smoke testing only, run:\n' >&2
  printf "UPDATE users SET dm_policy = 'everyone' WHERE user_name = '%s';\n" "$AYSE_USER" >&2
  printf 'Then rerun this script. No destructive cleanup was performed.\n' >&2
  exit 1
fi
expect_type 'chat.conversation.direct'

CONVERSATION_ID="$(extract_conversation_id)"
if [ -z "$CONVERSATION_ID" ]; then
  fail 'could not extract conversation public_id from direct conversation response'
fi
printf 'CONVERSATION_ID=%s\n' "$CONVERSATION_ID"

step '9. Send message'
request POST "/api/conversations/$CONVERSATION_ID/messages" '{
  "body": "Smoke test message"
}' send
expect_type 'chat.message.created'

step '10. List messages'
request GET "/api/conversations/$CONVERSATION_ID/messages" '' send
expect_type 'chat.messages.list'
if ! grep -q 'Smoke test message' "$BODY_FILE"; then
  fail 'expected sent message in message history'
fi

step '11. List conversations'
request GET '/api/conversations' '' send
expect_type 'chat.conversations.list'
if ! grep -q "$CONVERSATION_ID" "$BODY_FILE"; then
  fail 'expected conversation in conversation list'
fi

step '12. Logout'
request POST '/api/auth/logout' '' send-save
expect_type 'auth.logout'

step '13. Verify /api/me fails after logout'
request GET '/api/me' '' send
expect_code 'auth_required'

printf '\nBackend REST smoke test completed successfully.\n'
