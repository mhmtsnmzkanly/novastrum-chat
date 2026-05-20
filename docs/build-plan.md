# First Rebuild Build Plan

## 1. Executive Summary

The next implementation step is to create the clean rebuild skeleton, not the full product. The first phase should prove that the repository can host a Rust/Axum backend and a Vite/Tailwind frontend with basic build checks, logging, configuration, and later MariaDB connectivity.

Do not start chat, auth, WebSocket command handling, migrations, media upload, or community/discussion implementation in the first skeleton slice. The first goal is a stable foundation that future product slices can build on without mixing concerns.

## 2. Build Principles

- Use small commits.
- Keep one concern per commit.
- Do not implement chat before the server and web skeletons work.
- Do not implement WebSocket handling before the packet registry shape is settled in code.
- Do not add media upload early.
- Do not add compact packet mode early.
- Do not copy-paste legacy code without review.
- Keep `legacy/` read-only.
- Keep generated outputs out of git unless there is an explicit policy to commit them.
- Prefer boring, verifiable infrastructure over feature work in the first phase.

## 3. First Practical Build Slice

The first practical implementation slice should include:

- Create `server/` Rust project.
- Create `web/` Vite + Tailwind project.
- Add or update root README only if needed for the new skeleton.
- Add basic root `.gitignore` if needed.
- Add server health endpoint.
- Add server config loading.
- Add server tracing/logging.
- Add MariaDB pool initialization.
- Do not add auth yet.
- Do not add chat yet.
- Do not add WebSocket yet.
- Do not add WebSocket placeholder route unless it is clearly marked as deferred and does not implement protocol behavior.
- Add web static app shell.
- Verify Tailwind build.

Definition of done:

- `server/` compiles.
- Health endpoint exists.
- Config can be loaded from environment.
- MariaDB configuration and pool wiring are present by Commit B.
- `web/` builds.
- Documentation explains how to run the skeleton locally.
- No files under `legacy/` are modified.

## 4. Proposed Commit Sequence

### Commit A: Server Skeleton

Scope:

- Create `server/` Rust Axum skeleton.
- Add `/health` endpoint.
- Add tracing/logging initialization.
- Add config shell for host/port/app environment.
- Add basic application state if needed.
- No database connection yet.
- No auth.
- No chat.
- No WebSocket.

Expected validation:

- `cargo fmt`
- `cargo check`
- `cargo test` if tests exist
- `git status`

### Commit B: MariaDB Pool Shell

Scope:

- Add MariaDB config.
- Add SQLx MySQL/MariaDB pool initialization.
- Add `/health/db` or startup DB check.
- Keep migrations deferred unless a later dedicated schema step starts.
- No user/session schema yet.
- No repository implementations beyond pool wiring.

Expected validation:

- `cargo fmt`
- `cargo check`
- `cargo test` if tests exist
- DB health behavior documented for missing/unavailable database.
- `git status`

### Commit C: Web Skeleton

Scope:

- Create `web/` Vite + Tailwind skeleton.
- Add static app shell.
- Add basic stylesheet pipeline.
- Verify production build.
- Do not implement chat UI.
- Do not implement API client beyond a placeholder only if needed by the skeleton.

Expected validation:

- `npm install`
- `npm run build`
- `git status`

### Commit D: Development Docs

Scope:

- Add root development docs for running server and web.
- Include environment variables and local commands.
- Document that deployment is not included yet.
- Do not add Caddy/systemd production files yet.

Expected validation:

- Re-run relevant server/web checks if docs reference commands.
- `git status`

## 5. Server Skeleton Requirements

Expected future files:

```text
server/Cargo.toml
server/src/main.rs
server/src/app.rs
server/src/app_state.rs
server/src/config.rs
server/src/error.rs
server/src/http/mod.rs
server/src/http/routes.rs
server/src/http/health.rs
server/src/db/mod.rs
```

Initial responsibilities:

- `main.rs`: load config, initialize tracing, build app, bind listener, start server.
- `app.rs`: create Axum router.
- `app_state.rs`: shared app state shell.
- `config.rs`: environment-backed config structs.
- `error.rs`: common error/result shape if needed.
- `http/mod.rs`: HTTP module root.
- `http/routes.rs`: route assembly.
- `http/health.rs`: health endpoints.
- `db/mod.rs`: MariaDB pool wiring in Commit B.

Do not create domain modules such as `chat/`, `groups/`, or `discussion/` until their first implementation slice begins.

## 6. Server Dependencies

Commit A dependencies:

- `axum`: HTTP router and handlers.
- `tokio`: async runtime.
- `tracing`: structured logging API.
- `tracing-subscriber`: logging subscriber setup.
- `serde`: config and response serialization support if needed.
- `serde_json`: JSON responses if needed.
- `tower-http`: only if needed for tracing/cors/static middleware in the skeleton.

Commit B dependencies:

- `sqlx` with MySQL/MariaDB support and `runtime-tokio-rustls`.
- `dotenvy` or a chosen config strategy for local environment loading.

Later dependencies:

- `thiserror`: useful when domain/service errors start to grow.
- Password hashing/session/cookie crates: auth slice only.
- WebSocket feature/dependencies: WebSocket slice only.

Dependency rule: add dependencies when the commit actually uses them. Avoid adding auth, packet, media, or frontend-state dependencies early.

## 7. Web Skeleton Requirements

Expected future files:

```text
web/package.json
web/index.html
web/src/main.ts
web/src/styles.css
web/vite.config.ts
web/tailwind.config.*
web/postcss.config.*
web/src/App.ts
```

Notes:

- `web/dist/` is generated by build and should not be committed unless a future deployment policy says otherwise.
- Keep the first app shell static.
- Do not implement chat layout, auth flows, community feeds, or WebSocket clients in the first skeleton commit.
- If a framework is chosen later, file names may change, but the skeleton should still keep concerns separated.

## 8. Frontend Framework Decision

No final frontend framework decision is documented yet.

Options:

| Option | Notes |
| --- | --- |
| Plain TypeScript with Vite | Lowest framework commitment; good for a static shell and early API experiments. |
| React with Vite | Common choice for component-heavy chat UI; introduces framework decisions early. |
| Svelte with Vite | Good developer experience; also a framework commitment. |
| Other | Needs owner decision and reason before implementation. |

Recommendation for the first implementation slice: use Vite + TypeScript + Tailwind without a heavy framework, unless the owner chooses React or Svelte before implementation begins.

Unresolved choice: if the team wants a component framework from day one, the owner should choose before Commit C.

## 9. Build and Validation Commands

Implementation agents should run:

```sh
cargo fmt
cargo check
cargo test
npm install
npm run build
git status
```

Command notes:

- Run Cargo commands inside `server/` once `server/` exists.
- Run npm commands inside `web/` once `web/` exists.
- `cargo test` may be a no-op early but should still be attempted if tests exist.
- `npm install` should only happen during the web skeleton implementation, not during documentation planning.
- Do not install dependencies during this planning step.

## 10. Environment Configuration

Expected environment variables:

| Variable | Phase | Purpose |
| --- | --- | --- |
| `APP_ENV` | Commit A | `development`, `test`, or `production`. |
| `SERVER_HOST` | Commit A | Bind host, such as `127.0.0.1`. |
| `SERVER_PORT` | Commit A | Bind port, such as `3000`. |
| `DATABASE_URL` | Commit B | MariaDB connection string. |
| `SESSION_SECRET` | Later | Session signing/encryption secret once auth exists. |
| `REGISTRATION_MODE` | Later | `open`, `approval_required`, later `invite_only`. |
| `CORS_ALLOWED_ORIGIN` | Later if needed | Browser origin allowlist for split dev ports. |

Local config should fail with clear messages for missing required values. Commit A should not require `DATABASE_URL`; Commit B may require it only for DB health checks or pool startup, depending on chosen startup policy.

## 11. Migration Strategy

Recommendation:

- SQLx migrations should eventually live under `server/migrations/`.
- Do not create migration files in the first skeleton step.
- Convert `docs/database-design.md` into actual migrations in a dedicated later task.
- Keep schema creation separate from server skeleton and DB pool wiring.

Reason:

- The database design includes lifecycle and visibility rules that need careful migration ordering.
- The `conversation_memberships.visible_from_message_id -> messages.id` relationship needs deliberate schema handling.
- Creating migrations too early risks locking in half-reviewed table shapes.

## 12. Deployment Planning

Future deployment work should include:

- `Caddyfile`.
- `systemd` unit for the backend.
- Production `.env` or environment configuration policy.
- Web build artifact path, likely `web/dist`.
- Backend binary path.
- Database migration command policy.
- Health endpoint checks.
- `journalctl` logging notes.

Do not include deployment implementation in the first build slice. The first phase is local skeleton validation only.

## 13. Risks

| Risk | Mitigation |
| --- | --- |
| Dependency overgrowth | Add only dependencies used by the current commit. |
| Frontend framework choice too early | Use plain Vite + TypeScript + Tailwind unless owner chooses otherwise. |
| Generated UI sprawl | Start with a static shell only. |
| MariaDB connection failures blocking server startup | Decide whether DB is required at startup in Commit B and document behavior. |
| Committing `dist/` or `target/` | Add `.gitignore` rules before builds. |
| Accidental edits inside `legacy/` | Check `git status` before commit and stage only intended files. |
| Agent implementing chat too early | Treat chat/auth/WebSocket as out of scope for this build slice. |
| Packet registry drift | Do not start WebSocket commands before registry code shape is defined. |
| Skeleton overwriting root files | Inspect root before scaffolding and preserve docs/legacy. |

## 14. Stop Conditions

Implementation agent must stop and ask for direction if:

- Existing docs contradict this build plan.
- Package manager creates unexpected files outside `web/`.
- Cargo project creation would overwrite root files.
- Database connection assumptions are unclear.
- Frontend framework choice is unresolved and required for the selected scaffolding command.
- Any command tries to modify `legacy/`.
- The implementation scope drifts into auth, chat, WebSocket, migrations, media, communities, or discussion.
- Dependency installation fails in a way that suggests network or environment assumptions need owner approval.

## 15. Recommended First Implementation Prompt

```text
You are a senior Rust backend engineer.

Project:
mhmtsnmzkanly/novastrum-chat

Goal:
Create only Commit A from docs/build-plan.md.

Important:
- Do not create web/ yet.
- Do not install frontend dependencies.
- Do not create database migrations.
- Do not add MariaDB/SQLx yet.
- Do not implement auth, chat, groups, communities, discussion, WebSocket, media, or admin features.
- Do not modify legacy/.

Read first:
- docs/build-plan.md
- docs/target-architecture.md

Implement:
- Create server/ Rust Axum skeleton.
- Add /health endpoint.
- Add tracing/logging initialization.
- Add config shell for APP_ENV, SERVER_HOST, SERVER_PORT.
- Add basic AppState if useful.
- Keep handlers thin.

Expected files:
- server/Cargo.toml
- server/src/main.rs
- server/src/app.rs
- server/src/app_state.rs
- server/src/config.rs
- server/src/error.rs
- server/src/http/mod.rs
- server/src/http/routes.rs
- server/src/http/health.rs

Validation:
- Run cargo fmt inside server/.
- Run cargo check inside server/.
- Run cargo test inside server/ if tests exist.
- Run git status.

Commit:
- Stage only the server skeleton files and any necessary root .gitignore/README edits.
- Commit message: build: add server axum skeleton

Output:
- Report commit hash.
- Report created files.
- Report validation commands and results.
- Confirm no legacy files were modified.
```
