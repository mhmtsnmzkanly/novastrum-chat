# Target Architecture

This document defines the target architecture for the clean rebuild. It is architecture documentation only; it does not create implementation code or final database schema.

## 1. Executive Summary

Novastrum should be rebuilt as a modular monolith: one Rust/Axum backend, one Vite/Tailwind frontend, and one MariaDB database. The backend owns all business rules, visibility checks, transactions, and realtime fanout decisions. The frontend owns presentation, user interaction, local UI state, API calls, and WebSocket packet handling.

The database is the source of truth. WebSocket delivery is an optimization for realtime updates, not the source of truth. REST handles bootstrap, CRUD, history, settings, and pagination. WebSocket handles authenticated realtime packets. The packet protocol starts in readable mode and can later add compact mode from the same packet registry.

Product boundaries are explicit:

- Direct messages are private.
- Group chats are private to members.
- Users only see groups they are members of.
- New group members cannot see old group messages.
- Communities are Reddit-like topic spaces.
- Community-visible content is discussion content, not open chat.
- Discussion posts belong to communities.
- A combined discussion feed replaces any mandatory General community.

## 2. Repository Layout

Proposed future root layout:

```text
/
├── web/
├── server/
├── docs/
└── legacy/
```

Responsibilities:

- `web/`: Vite frontend app, Tailwind setup, UI components, API client, WebSocket client, frontend state.
- `server/`: Rust backend, Axum HTTP/WebSocket edge, services, repositories, migrations, config.
- `docs/`: product, architecture, API, database, and build planning documents.
- `legacy/`: moved old project, kept for reference only.

## 3. Backend Architecture

Backend stack:

- Rust.
- Tokio runtime.
- Axum for HTTP routes and WebSocket upgrade.
- SQLx with MySQL/MariaDB support.
- MariaDB connection pool.
- Tracing/logging through `tracing`-style structured events.

Core backend parts:

- Axum HTTP/WebSocket edge: owns routing, request extraction, response conversion, middleware, and WebSocket upgrade.
- `AppState`: shared immutable/runtime application state passed through handlers.
- Service layer: owns business rules, authorization checks, status checks, transactions, and orchestration.
- Repository layer: owns SQL and row mapping.
- MariaDB pool: shared database pool used by repositories and transaction scopes.
- `WsHub`: owns live WebSocket connections and fanout to currently connected clients.
- Packet protocol modules: readable packet structs, packet type registry, error packets, future compact mappings.
- Config module: environment/config file loading, validation, deployment-sensitive values.
- Error module: domain errors, validation errors, auth errors, forbidden/not-found/rate-limit errors, packet/REST conversion.
- Tracing/logging: structured logs around requests, auth failures, write operations, WebSocket lifecycle, and internal errors.

The service layer should be the main boundary for correctness. Handlers should stay thin and repositories should stay SQL-focused.

## 4. Backend Module Layout

Proposed future backend layout:

```text
server/src/main.rs
server/src/app.rs
server/src/app_state.rs
server/src/config.rs
server/src/error.rs
server/src/http/
server/src/ws/
server/src/auth/
server/src/users/
server/src/chat/
server/src/groups/
server/src/communities/
server/src/discussion/
server/src/notifications/
server/src/media/
server/src/admin/
server/src/db/
```

Suggested responsibilities:

- `main.rs`: process entrypoint, config load, logging init, app boot, graceful shutdown.
- `app.rs`: Axum router construction and top-level middleware wiring.
- `app_state.rs`: shared `AppState` definition and initialization.
- `config.rs`: config structs, env loading, validation.
- `error.rs`: application error types and response/packet conversion.
- `http/`: route modules, request/response DTOs, extractors, pagination helpers.
- `ws/`: WebSocket upgrade, connection task, `WsHub`, packet send/receive, heartbeat/reconnect support.
- `auth/`: login/logout/session service, password hashing boundary, auth middleware support.
- `users/`: user identity, status, DM policy, user settings.
- `chat/`: direct conversation and message services shared by private direct chat.
- `groups/`: group creation, group roles, membership lifecycle, visibility boundary rules.
- `communities/`: community topic spaces, visibility, admin-created community management.
- `discussion/`: posts, replies, votes, combined feed queries.
- `notifications/`: notification records, unread state, later presence notifications.
- `media/`: deferred upload preparation/storage policy boundary.
- `admin/`: admin-only workflows and moderation tooling.
- `db/`: pool construction, migration integration, transaction helpers, SQLx repository shared types.

## 5. Layer Rules

Handlers:

- Parse HTTP requests and WebSocket packets.
- Call services.
- Convert service results into REST responses or packets.
- Do not contain business rules.
- Do not contain SQL.
- Do not manually enforce complex authorization beyond calling the correct service/auth extractor.

Services:

- Own business rules.
- Own product invariants.
- Own authorization decisions.
- Own transaction boundaries.
- Call repositories.
- Decide when to publish realtime events after successful commits.

Repositories:

- Own SQL.
- Map database rows to repository/domain structs.
- Do not decide product rules.
- Do not publish WebSocket events.

`WsHub`:

- Owns live connections only.
- Tracks connection/session ids needed for delivery.
- Sends packets to connected clients.
- May maintain in-memory connection routing state.
- Must not write to the database.
- Must not be the source of truth for membership, history, unread state, or authorization.

Database:

- Source of truth for users, sessions, conversations, group memberships, messages, communities, discussion, notifications, and audit-relevant records.
- Reconnect/resync reads from database-backed history, not from `WsHub`.

## 6. Domain Boundaries

Direct chat:

- Private conversation between participants.
- Access is controlled by participant records and DM policy.
- Direct messages are not community content.

Group chat:

- Private chat among members.
- Groups are not public discovery spaces.
- Group list/detail/history are visible only to members.
- Group history is constrained by each membership period's join boundary.

Community discussion:

- Communities are topic spaces.
- Discussion posts and replies belong to communities.
- Public/community-visible content is discussion content.
- A combined feed aggregates visible community discussion posts.

Community open chat excluded initially:

- It overlaps with group/direct chat but has different moderation and abuse risks.
- It complicates notification, mute, realtime volume, and visibility rules.
- The owner decision is that public community content is discussion, not open chat.

Discussion belongs to communities because:

- Communities provide topic, visibility, moderation, and feed scope.
- A mandatory General community is not needed.
- Global discovery should be a feed query across visible communities, not a special chat/channel.

## 7. Realtime Architecture

WebSocket endpoint:

- Single authenticated endpoint, likely under `/ws`.
- Caddy proxies `/ws` to the backend.
- The server authenticates the connection using the same session model as HTTP.

Readable packet mode:

- Initial/default development mode.
- Server packet shape: `id`, `type`, `state`, `data`.
- Client packet shape: `req`, `type`, `data`.
- `data` is an object.
- Frontend routes handlers by `type`.

Compact packet mode later:

- Compact shape: `i`, `t`, `s`, `d`.
- `d` is an array.
- Requires strict registry-defined field order.
- Should be generated from the same packet registry as readable mode.
- Should target WebSocket first, not REST.

Packet registry:

- Durable schema source before WebSocket implementation.
- Maps readable type strings to compact numeric ids.
- Reserves domains: auth/user, chat/direct, group, community, discussion, notification/presence, error/system.
- Defines payload fields and compact `d` ordering.

Request/reply correlation:

- Client commands include `req`.
- Server replies include server `id` and original `req`.
- Server-originated events do not require `req`.

Event id and idempotency:

- Server events include stable packet ids.
- Domain records also include stable ids such as message id or post id.
- Frontend deduplicates by event id and/or domain object id.

Reconnect/resync direction:

- History endpoints are the recovery path.
- WebSocket reconnect should trigger resync by cursor, timestamp, sequence, or last known ids.
- Group resync must enforce membership and join-boundary visibility.
- `WsHub` cannot replay from memory as the authority.

## 8. API Architecture

REST:

- Used for bootstrap, current user, login/logout, CRUD, history, pagination, settings, admin actions, and initial page data.
- Uses normal HTTP status for transport-level success/failure.
- Uses packet-inspired readable response bodies for application state.

REST readable response shape:

```json
{
  "id": "res_01J...",
  "req": "req_01J...",
  "type": "auth.me",
  "state": "ok",
  "data": {}
}
```

WebSocket:

- Used for realtime delivery and later client commands where useful.
- Uses packet envelopes directly.
- Starts readable; compact can be added later.

Error format:

```json
{
  "id": "err_01J...",
  "req": "req_01J...",
  "type": "error.forbidden",
  "state": "error",
  "data": {
    "code": "forbidden",
    "message": "You are not allowed to perform this action."
  }
}
```

Validation format:

```json
{
  "id": "err_01J...",
  "req": "req_01J...",
  "type": "error.validation",
  "state": "error",
  "data": {
    "code": "validation_failed",
    "fields": [
      {
        "path": "body",
        "code": "too_long",
        "message": "Message body is too long."
      }
    ]
  }
}
```

Pagination format:

- Use cursor pagination for messages, feeds, discussion lists, and notification lists.
- Response `data` should include `items`, `next_cursor`, and optionally `has_more`.
- Cursor format should be opaque to the frontend unless a later API contract explicitly exposes it.

## 9. Database Architecture

This section describes domain table areas only. Full schema belongs in `docs/database-design.md`.

Domain tables:

- `users`: account identity, `user_name`, `public_name`, status, registration state, `dm_policy`.
- `sessions`: authenticated sessions, expiry, invalidation, device metadata if retained.
- `conversations`: direct/private conversation records. Community discussion should not be modeled as open chat.
- `conversation_members`: direct conversation participants and possible generic participant records.
- `messages`: direct/group message records, deletion metadata, conversation/group linkage, created ordering.
- `groups`: private group metadata.
- `group_members`: role (`owner`/`member`), joined/left/removed lifecycle, active status, `visible_from_message_id` or equivalent boundary.
- `communities`: topic spaces, visibility state, admin-created records.
- `community_members` or `community_follows`: optional personalization/notifications, not required for reading public communities.
- `discussion_posts`: top-level community posts.
- `discussion_replies`: replies linked to posts or parent replies, depending on final thread model.
- `discussion_votes`: one user's vote/reaction per target, depending on final vote rule.
- `notifications`: later notification records and unread state.
- `packet_registry`: optional runtime table. A docs/schema source may be enough; if runtime inspection is needed, a table can mirror the registry.
- `media_files`: later upload metadata and storage references.
- `moderation/audit`: later action logs, moderation decisions, and possibly separate deleted-content retention if chosen.

MariaDB considerations:

- Design indexes for membership checks, history pagination, group boundary filtering, and combined feed queries.
- Avoid relying only on timestamps for group join boundaries if ids or sequences are safer.
- Use transactions for multi-step writes such as message send, group member add/remove, and discussion vote changes.

## 10. Authorization Model

Authenticated app access:

- Protected app routes and API routes require a valid session unless explicitly public.
- Banned and deleted users must not gain authenticated app access.

User status checks:

- `active`: can read/write according to permissions.
- `pending`: behavior still needs product decision; do not assume full access.
- `suspended`: can log in/read but cannot write.
- `banned`: cannot log in.
- `deleted`: hidden/deactivated.

DM policy checks:

- Recipient `dm_policy` is enforced before creating direct conversations or sending first messages.
- Supported policies: `everyone`, `shared_group_members`, `friends_only`, `none`.
- `friends_only` is reserved/deferred if friendships are not implemented.

Group membership checks:

- Group list, detail, member list, and messages require current active membership or a precisely defined historical access rule.
- Owner-only actions include add/remove member initially.
- Members can leave.
- Group max size is 10.

Group visibility boundary checks:

- Message history must filter by member visibility boundary.
- Rejoin creates a new boundary.
- Realtime delivery and reconnect/resync must apply the same boundary rules.

Community post visibility:

- Public communities are readable without joining.
- Posting/replying requires authentication and write permission.
- Private communities are deferred.

Admin-only community creation:

- Community creation starts as admin-only.
- Community owner/moderator roles are deferred.

## 11. Frontend Architecture

Frontend stack:

- Vite build.
- Tailwind build.
- TypeScript is recommended for packet contracts and state safety, but final tooling details belong in build planning.

Gemini Canvas HTML:

- Treat Gemini Canvas HTML as design input or prototype reference only.
- Do not paste generated HTML as final app architecture.
- Extract intent, layout ideas, and interactions into components.

Frontend structure:

- Component separation by product area: auth, app shell, direct chat, groups, communities, discussion, notifications, admin/settings later.
- API client module for REST packet-inspired responses.
- WebSocket client module for connection lifecycle, readable packet handling, request/reply correlation, reconnect hooks, and later compact support.
- State model separated from rendering; frontend state caches server truth but does not own business rules.
- Packet handler registry routes readable packets by `type`.
- Later compact support maps numeric `t` to the same handler registry through packet schema metadata.

Frontend rule:

- The frontend may hide unavailable actions, but backend services enforce all rules.
- No business rules should live only in frontend code.

## 12. Deployment Architecture

Target deployment:

- Ubuntu server.
- Caddy reverse proxy.
- Backend runs as a `systemd` service.
- MariaDB runs as a local service.
- Frontend builds to `web/dist`.

Caddy:

- Serves `web/dist` for the web app.
- Proxies `/api` to the backend.
- Proxies `/ws` to the backend with WebSocket upgrade support.
- Owns TLS and HTTP-to-HTTPS redirect.

Backend service:

- Runs as an unprivileged systemd service user.
- Loads `.env`/environment config.
- Exposes health endpoint.
- Writes structured logs to stdout/stderr for `journalctl`.

Database:

- MariaDB local service.
- SQLx MySQL/MariaDB pool.
- Migration runner decision still needed: startup migrations, CLI-run migrations, or deployment-run migrations.

Config:

- Use environment/config for database URL, bind address, cookie/session secrets, CORS/host settings, registration mode, and future media settings.
- Secrets must not be committed.

Operations:

- Health endpoint supports deployment checks.
- Logs inspected with `journalctl`.
- Backup/restore policy belongs in operations docs and must include MariaDB and later media storage.

## 13. What Not To Do

- No microservices now.
- No Redis/queue early.
- No public group discovery.
- No community chat early.
- No file upload before policy is clear.
- No compact-only packet format at first.
- No SQL inside handlers.
- No business rules inside frontend.
- No WebSocket-as-source-of-truth design.
- No mandatory General community.
- No broad admin dashboard before focused moderation workflows.
- No generated/prototype HTML treated as final frontend architecture.
- No schema decisions based solely on legacy PostgreSQL assumptions; target DB is MariaDB.

## 14. Architecture Risks

Packet protocol overengineering:

- Risk: building registry/generation/compact mode before the app needs it.
- Mitigation: readable mode first, stable minimal registry before WebSocket coding, compact later.

Group history boundary bugs:

- Risk: leaking pre-join messages to new or rejoined members.
- Mitigation: model membership periods and query boundaries explicitly; test history and resync paths.

DM policy complexity:

- Risk: conversation creation, first message, existing conversation behavior, and future friendships conflict.
- Mitigation: enforce policy in one service and defer `friends_only` until friendship exists.

Community/discussion scope creep:

- Risk: adding community chat, private communities, owner roles, advanced moderation, and feeds too early.
- Mitigation: start with public communities and discussion posts only.

Media upload security:

- Risk: path traversal, malware, quotas, private file leaks, backup exposure.
- Mitigation: defer media until policy and storage model are clear.

MariaDB query/index mistakes:

- Risk: poor cursor pagination, slow feed queries, weak membership filters.
- Mitigation: database design doc must specify indexes and query shapes for MariaDB.

Frontend generated-code sprawl:

- Risk: prototype HTML/CSS grows into unmaintainable app code.
- Mitigation: Vite/Tailwind component architecture; prototypes are design input only.

## 15. First Build Slice

This is the first practical build slice, not a release label:

1. Create `server/` skeleton.
2. Add backend config loading and validation.
3. Add tracing/logging initialization.
4. Add Axum app/router construction.
5. Add health endpoint.
6. Add MariaDB pool initialization.
7. Decide migration runner strategy, but do not design all schema yet.
8. Prepare database design document for user/session tables later.
9. Do not build chat yet.
10. Create `web/` skeleton.
11. Add Vite build.
12. Add Tailwind build.
13. Create static authenticated-app shell placeholder.
14. Add frontend API client shape only after API contract is documented.
15. Do not implement WebSocket protocol until packet registry and API contract docs are ready.

The slice proves build/deploy structure and runtime plumbing before domain complexity.

## 16. Recommended Next Documents

- `docs/database-design.md`: schema, indexes, migrations, MariaDB constraints, transaction boundaries.
- `docs/api-contract.md`: REST endpoints, response packets, errors, pagination, auth behavior.
- `docs/websocket-protocol.md`: packet registry, readable schemas, request/reply, reconnect/resync, compact roadmap.
- `docs/build-plan.md`: practical sequence for creating `server/` and `web/` without over-scoping.
