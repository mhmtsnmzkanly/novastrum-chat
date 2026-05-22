# Local Development Guide

## 1. Prerequisites

Install:

- Rust toolchain with Cargo.
- Node.js and npm.
- MariaDB is optional for now.

The server can start without MariaDB because `DATABASE_URL` is optional in the current rebuild slice.

## 2. Repository Layout

```text
server/   Rust + Axum backend skeleton
web/      Vite + React + TypeScript + Tailwind frontend skeleton
docs/     Planning and architecture documentation
legacy/   Historical reference copy of the old project
```

`legacy/` must remain read-only during normal rebuild work.

## 3. Server Development

Run server checks from `server/`:

```sh
cd server
cargo fmt
cargo check
cargo test
cargo run
```

The server defaults to `127.0.0.1:8080`.

## 4. Server Environment Variables

| Variable | Default | Required Now | Purpose |
| --- | --- | --- | --- |
| `APP_ENV` | `development` | No | Runtime environment label. |
| `SERVER_HOST` | `127.0.0.1` | No | Server bind host. |
| `SERVER_PORT` | `8080` | No | Server bind port. |
| `DATABASE_URL` | None | No | MariaDB connection string for DB health checks. |

`DATABASE_URL` is optional for now. Missing database configuration should not stop local server startup.

## 5. Health Checks

Service health:

```text
GET /health
```

Expected response:

```json
{
  "state": "ok",
  "service": "novastrum-server",
  "app_env": "development"
}
```

Database health:

```text
GET /health/db
```

If `DATABASE_URL` is configured and the database ping succeeds:

```json
{
  "state": "ok",
  "database": "mariadb"
}
```

If `DATABASE_URL` is missing:

```json
{
  "state": "error",
  "database": "mariadb",
  "message": "DATABASE_URL is not configured"
}
```

`/health` works without database configuration. `/health/db` reports database status without panicking or leaking database credentials.

## 6. Web Development

Run web commands from `web/`:

```sh
cd web
npm install
npm run dev
npm run build
npm run typecheck
```

The current web app is a static skeleton only. It does not call the backend.

## 7. Current Limitations

Not implemented yet:

- Auth.
- Chat.
- WebSocket.
- Database migrations.
- Real API client.
- Production deployment files.
- Routing library.
- Frontend state management library.

## 8. Git Hygiene

Rules:

- Do not edit `legacy/`.
- Do not commit `server/target/`.
- Do not commit `web/node_modules/`.
- Do not commit `web/dist/`.
- Stage only files that belong to the current task.
- Check `git status` before every commit.

The root `.gitignore` already excludes current build outputs.

## 9. Next Likely Implementation Steps

Likely next slices:

- Convert `docs/database-design.md` into migration files.
- Add auth/session schema and service boundaries.
- Add API response helpers that match `docs/api-contract.md`.
- Add frontend API client later, after REST endpoints exist.
- Add WebSocket protocol code later, after packet registry shape is settled in code.

These are likely next steps, not authorization to implement them in this documentation task.

## 10. Troubleshooting

### `DATABASE_URL` Missing

This is expected for now. The server should still start and `/health` should return `state: ok`. `/health/db` should return:

```json
{
  "state": "error",
  "database": "mariadb",
  "message": "DATABASE_URL is not configured"
}
```

Set `DATABASE_URL` only when you want to test database connectivity.

### Port Already In Use

Change `SERVER_PORT`:

```sh
cd server
SERVER_PORT=18080 cargo run
```

Then use:

```text
http://127.0.0.1:18080/health
```

### `npm install` Issues

Check:

- Node.js is installed.
- npm is installed.
- You are running the command from `web/`.
- Network access is available for package downloads.

Then retry:

```sh
cd web
npm install
```

### `cargo check` Dependency Issues

Check:

- Rust and Cargo are installed.
- You are running the command from `server/`.
- Network access is available if Cargo needs to download crates.

Then retry:

```sh
cd server
cargo check
```

If dependency resolution continues to fail, capture the exact error before changing dependencies.
