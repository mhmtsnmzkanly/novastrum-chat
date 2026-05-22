# Novastrum Chat

Novastrum Chat is in a clean rebuild phase. The current repository contains a new Rust/Axum server skeleton, a Vite + React + TypeScript + Tailwind web skeleton, planning documents, and the old project preserved under `legacy/`.

## Project Status

Current implementation status:

- Server skeleton exists under `server/`.
- `/health` and `/health/db` endpoints exist.
- Web skeleton exists under `web/`.
- MariaDB configuration is optional for now.
- Auth, chat, WebSocket, migrations, and real API integration are not implemented yet.

## Repository Layout

```text
server/   Rust + Axum backend skeleton
web/      Vite + React + TypeScript + Tailwind frontend skeleton
docs/     Product, architecture, API, database, protocol, and build planning
legacy/   Historical reference copy of the old project
```

`legacy/` is historical reference only. Do not edit it during the rebuild unless a migration task explicitly says to.

## Run The Server

```sh
cd server
cargo run
```

Defaults:

- `APP_ENV=development`
- `SERVER_HOST=127.0.0.1`
- `SERVER_PORT=8080`
- `DATABASE_URL` is optional for now

Health checks:

- `GET http://127.0.0.1:8080/health`
- `GET http://127.0.0.1:8080/health/db`

## Run The Web App

```sh
cd web
npm install
npm run dev
```

Build checks:

```sh
npm run build
npm run typecheck
```

## Planning Docs

Planning documents live in `docs/`. Start with:

- [docs/build-plan.md](docs/build-plan.md)
- [docs/target-architecture.md](docs/target-architecture.md)
- [docs/database-design.md](docs/database-design.md)
- [docs/api-contract.md](docs/api-contract.md)
- [docs/websocket-protocol.md](docs/websocket-protocol.md)
- [docs/product-rules.md](docs/product-rules.md)
