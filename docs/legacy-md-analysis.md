# Legacy Markdown Analysis

This analysis covers every Markdown file found under `legacy/` after the legacy move. It is limited to documentation analysis and does not make rebuild architecture decisions.

## 1. Markdown File Inventory

| Path | Apparent purpose | Status |
| --- | --- | --- |
| `legacy/SPECS.md` | Original full system blueprint for a single-node Rust/PostgreSQL/WebSocket community platform. | Partly stale. Strong intent signal, but some production-readiness and comment mandates should not be carried forward blindly. |
| `legacy/API_WEBSOCKET_GUIDE.md` | Root API and WebSocket guide with endpoint groups, event flow, client integration notes, and payload examples. | Mostly current as legacy reference. Shorter than `legacy/docs/API_WEBSOCKET_GUIDE.md` and omits some later routes. |
| `legacy/DESIGN_BRIEF.md` | Root frontend brief for a single-page chat/discussion/notifications UI against existing backend contracts. | Mostly current as frontend intent, but its note that chat history API is not implemented conflicts with later docs and routes. |
| `legacy/EKSIK.md` | Root missing-items list in Turkish, stating only production captcha and Caddy/TLS deployment remain from SPECS. | Partly current, but too narrow because other docs list UX, ops, API schema, backup restore, and WebSocket idempotency gaps. |
| `legacy/TASKLIST.md` | Practical TODO list for raw UI, DM request events, file upload UI, chat history, presence/unread, and cleanup. | Current or unclear. It likely captures active product/UX gaps, but some backend endpoint statements appear stale. |
| `legacy/docs/API_WEBSOCKET_GUIDE.md` | Concise integration reference for frontend and integration teams. | Mostly current. Route scan confirms many endpoints listed here exist in legacy source. |
| `legacy/docs/BACKEND_FEATURES.md` | Backend feature and API integration summary compiled from routes and docs. | Mostly current as a backend inventory, though it should still be verified against handler behavior before reuse. |
| `legacy/docs/DESIGN_BRIEF.md` | Condensed design brief summarizing API, WebSocket, state, UX, and security notes. | Mostly current as an overview. |
| `legacy/docs/EKSIK.md` | Prioritized missing-items summary for captcha, deployment, backup restore/retention, WebSocket idempotency, API docs, and upload security. | Current as rebuild warning list. |

Markdown files found and analyzed: 9.

Markdown files that could not be read: none.

## 2. Project Intent Extracted From Legacy Docs

Novastrum was intended as a real-time community platform with chat, direct messages, group chats, community messages, forum-like discussion threads, notifications, file upload, admin moderation, and operational backup support.

The core product flows were:

- Captcha gate before login/register/reset/PIN.
- Registration into `Pending` status, followed by admin approval before active use.
- Session-backed authenticated app access through signed HttpOnly cookies.
- Direct chat only after friendship; non-friends start with DM requests.
- Group chat with a per-user group creation limit.
- Community chat visible to everyone, with mute support and mention override.
- Discussion threads with replies, voting, cursor pagination, and moderated edit requests.
- Real-time updates over WebSocket for chat, discussion, notification, and admin events.
- Admin workflows for approving/rejecting users, status changes, permissions, rate config, audit logs, and discussion edit decisions.

The rebuild should preserve the product intent, not necessarily the legacy implementation shape.

## 3. Existing Architecture Notes

The legacy blueprint specifies:

- Single-node deployment.
- Rust backend.
- PostgreSQL database on the same machine.
- Static HTML/CSS/JS frontend served from `web/`.
- WebSocket endpoint at `/ws`.
- Local file storage.
- No Docker, Redis, or external queue in the original plan.
- Event-driven design, but explicitly not event-sourced.
- Database as source of truth.
- Event table as audit log only, with no replay or snapshots.
- Transaction integrity for writes, especially message send.
- Permission-based moderation rather than role enum.
- `u8` event types, with `0` and `255` reserved.

The actual legacy source layout matches the blueprint broadly: `src/main.rs` wires modules for auth, websocket, events, permissions, rate limiting, presence, chats, community, discussion, notifications, files, logging, backup, and health.

## 4. Existing Setup/Deployment Notes

Documented setup/deployment assumptions:

- Ubuntu 24.04 LTS.
- Caddy reverse proxy.
- Strict TLS and HTTP-to-HTTPS redirect.
- Rust backend without Docker.
- PostgreSQL on the same machine.
- Local file storage.
- Environment-driven config, including `DATABASE_URL`, cookie/session secrets, captcha config, and storage root.
- `/health` endpoint for health checks.
- Structured JSON logs and panic hook are part of the intended monitoring baseline.

Important gap: multiple docs state that Caddyfile, systemd unit, TLS automation, restore playbook, secret management docs, and production deployment docs are missing.

## 5. Existing Database Notes

Database-related notes from the docs:

- PostgreSQL is the source of truth.
- Sessions are DB-backed with 7-day max lifetime and 24-hour inactivity timeout.
- Active WebSocket presence is tracked through `user_sessions`.
- User status values are `Pending`, `Active`, `Suspended`, and `Banned`.
- Separate message tables are intended for DM, group, and community messages.
- Soft-deleted messages keep no original message body; body becomes `Mesaj silindi`, with deleted flag set.
- Mentions are stored as `:mention=USER_ID`.
- Rate limits are stored in `rate_config`.
- Permissions are table-driven.
- Events are inserted inside the transaction, then dispatched asynchronously after commit.
- Notifications include unread state, per-chat mute, and `mute_until`; mentions override mute.
- Activity logs are intended to be monthly partitioned with no retention deletion.
- Backups are table-by-table `pg_dump` exports under `dump/YYYY-MM-DD-HH/`, optionally zipped.
- Cursor pagination uses time-based cursors, commonly `WHERE created_at < ? ORDER BY created_at DESC LIMIT 50`.
- Legacy docs mention automatic database creation from `DATABASE_URL` for local/developer convenience.

## 6. Existing Frontend Notes

Frontend intent:

- `index.html`: captcha gate.
- `auth.html`: login, register, reset, and PIN.
- `app.html`: main user app.
- `admin.html`: admin UI.
- `melt.css` and `melt.js`: local styling/helper layer.
- `connection.js`: WebSocket/client command layer in newer frontend notes.

Expected user UI:

- Segmented chat list for Direct, Groups, and Community.
- Message feed with body, time, edited/deleted state, and sender-only edit/delete within 300 seconds.
- DM request inbox with accept/reject.
- Group creation.
- New DM request flow.
- Discussions tab with create thread, list posts, voting, and pagination.
- Notifications panel, mute support, presence/unread counters where available.
- File upload flow through prepare then multipart upload.

Frontend helper expectations:

- `Melt.api(path, method, payload)` wraps fetch and sends credentials.
- `Melt.uploadFile(file, { user_id, file_id })` handles multipart upload after prepare.
- `Melt.highlightMentions(text)` renders mentions.
- `Melt.toast`, `Melt.showNotice`, and `Melt.hideNotice` handle lightweight UI feedback.
- `Melt.startPresenceSocket(elementId)` manages WebSocket connection, ping, reconnect, and presence status UI.
- `Melt.onWebSocketEvent` registers real-time handlers.

The docs repeatedly emphasize that API responses use `ApiEnvelope`; frontend should check HTTP status, then `result.ok`, then `result.body.ok`, then data.

## 7. Existing Backend Notes

Documented endpoint groups include:

- Auth/session: captcha verify, register, login, reset, PIN, logout, logout all.
- Bootstrap/health: `/api/bootstrap/state`, `/health`.
- Chat: send, edit, delete, message history, DM request, DM request list, accept, reject, group create/rename/member add/member remove/delete.
- Community: mute community notifications.
- Discussion: post, vote, edit request, list.
- Files: prepare and multipart upload.
- Notifications: list and unread.
- Presence: online status.
- Backup: backup plan.
- Admin: overview, users, approve, reject, status, permission toggle, rate config, audit, discussion edit list/decision.

The route scan of `legacy/src/main.rs` confirms the endpoint names listed in the newer docs, including chat history, group member endpoints, unread notifications, presence online, backup plan, and discussion edit moderation routes.

Important behavior rules:

- Message send rate limit: 1 message per 3 seconds.
- DM start limits: 2/minute, 10/hour, 30/day.
- Vote rate limit: about 1/minute.
- Discussion reply depth: max 3.
- Group creation: max 3 per user.
- Message edit: sender-only, within 300 seconds.
- Message delete: sender-only soft delete.
- Suspended users can log in and read, but cannot write.
- Banned users cannot log in/read/write.

## 8. Existing WebSocket/Realtime Notes

WebSocket endpoint:

- `GET /ws`, authenticated by signed session cookie.

Documented server events:

- `chat.message`
- `discussion.post`
- `discussion.vote`
- `notification.created`
- `admin.event`

Documented client rules:

- Treat UI event handling as idempotent.
- Validate event type and required payload identifiers before changing UI.
- Use reconnect with exponential backoff.
- Resync minimal state after reconnect where needed.

Known realtime gap:

- Docs recommend adding `event_id` and optional `version` to WebSocket payloads for idempotency and schema migration.
- `TASKLIST.md` says DM accept/reject should push WebSocket notification/admin events so frontends do not depend on polling.

## 9. Security Notes

Security-related documentation:

- Signed HttpOnly cookie sessions.
- Fetch calls should use same-origin credentials.
- Password hashing is Argon2id.
- Password policy in original SPECS is only `a-z0-9`; this is a weak legacy constraint and should be reconsidered during rebuild analysis.
- Production captcha is missing; current handler uses a development secret.
- Admin pages require admin checks before serving.
- Permission system is table-driven, not role enum based.
- File upload requires MIME, size, and permission checks.
- Upload docs call out path traversal prevention, storage root confinement, quota/retention, and optional scanning as concerns.
- Deployment docs are missing for TLS, reverse proxy, Caddy/systemd, and secret management.

## 10. Broken/Stale/Contradictory Information

- `legacy/SPECS.md` claims the system is a production baseline and includes Caddy TLS in the target architecture, but both `legacy/EKSIK.md` and `legacy/docs/EKSIK.md` say production captcha and Caddy/TLS deployment are still missing.
- `legacy/SPECS.md` says every function, operation, and logical step must contain explanatory comments. That is not a maintainable rebuild rule and should be treated as historical instruction, not technical architecture.
- `legacy/DESIGN_BRIEF.md` says chat history API is not implemented, while `legacy/docs/API_WEBSOCKET_GUIDE.md`, `legacy/docs/BACKEND_FEATURES.md`, and `legacy/src/main.rs` list `/api/chats/message/history`.
- `legacy/TASKLIST.md` asks for server-side pagination/history API or a temporary limit for raw UI. This may be stale for backend existence, but still relevant if the raw UI does not consume the endpoint well.
- `legacy/EKSIK.md` says only two SPECS items remain. `legacy/docs/EKSIK.md` lists additional unresolved operational and integration issues: backup restore/retention, WebSocket idempotency, API examples/OpenAPI, and upload security.
- Frontend docs differ on helper ownership: root `DESIGN_BRIEF.md` mentions `connection.js` commands, while API guides emphasize `Melt.startPresenceSocket` and `Melt.onWebSocketEvent`. Both may exist, but rebuild should not assume either abstraction without reviewing actual frontend.
- UI docs mention no external CDN, while `TASKLIST.md` references a Tailwind CDN warning in App-Fixed. That suggests older prototype UI files may not match intended production constraints.

## 11. Reusable Ideas

- Preserve the domain split: auth, sessions, chats, discussions, notifications, presence, files, admin, permissions, rate limiting, events, backup, health.
- Keep database-as-source-of-truth and event table as audit log, not replayable event sourcing.
- Keep transaction-first write boundaries for message send and other state changes.
- Keep explicit status behavior for pending/active/suspended/banned users.
- Keep permission-table concept, but revisit ergonomics and admin UX.
- Keep signed HttpOnly cookie sessions unless a later auth analysis finds a better fit.
- Keep cursor pagination for discussion, notifications, and chat history.
- Keep real-time event names as migration vocabulary, but add event IDs/versioning if retained.
- Keep prepare-then-upload file flow.
- Keep admin discussion edit moderation as a concept.
- Keep clear `ApiEnvelope` or replace it with an equally explicit response standard.

## 12. Things To Avoid In The Rebuild

- Do not treat `legacy/SPECS.md` as fully current or automatically authoritative.
- Do not carry over the "comment every logical step" rule.
- Do not launch production with development captcha behavior.
- Do not ship without deployment/TLS/secret-management documentation.
- Do not rely only on WebSocket streams without reconnect resync and idempotency.
- Do not implement chat feeds as append-only without history pagination.
- Do not leave file upload policy vague around MIME validation, storage quota, scanning, cleanup, and path safety.
- Do not assume the old raw HTML prototypes are the target frontend architecture.
- Do not bake in the weak legacy password policy without a security review.
- Do not store generated backup dumps in the application repo as a production pattern.

## 13. Open Questions

- What is the intended rebuild stack: keep Rust backend, or only preserve product behavior?
- Should the rebuild remain single-node, or should deployment constraints be revisited?
- Which legacy UI file is the closest behavioral reference: `web/app.html`, `web/raw-app.html`, `UI/App.html`, or `UI/App-Fixed.html`?
- Should `ApiEnvelope` remain, or should the rebuild use a typed OpenAPI-first contract?
- Should WebSocket payloads gain `event_id`, `version`, and replay/resync support?
- What is the correct production captcha provider?
- What is the restore/retention/encryption policy for backups?
- Which permission model should exist on day one of the rebuild?
- Should usernames remain immutable and exact-match mention based?
- What migration, if any, is expected for existing SQL dumps?

## 14. Recommended Next Steps

1. Review legacy source routes and handlers against the Markdown endpoint inventory.
2. Inspect `legacy/migrations/0001_init.sql` and dump samples to produce a database/domain model analysis.
3. Inspect `legacy/web/`, `legacy/UI/`, and `legacy/Raw-App Demo.png` to separate useful UX behavior from stale prototypes.
4. Inspect `legacy/src/websocket`, `legacy/src/events`, and `legacy/src/notifications` to document realtime behavior and gaps.
5. Inspect `legacy/src/auth`, `legacy/src/permissions`, and `legacy/src/files` for security-sensitive rebuild requirements.
6. Create a rebuild requirements document after source analysis, not before.
