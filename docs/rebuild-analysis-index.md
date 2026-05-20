# Rebuild Analysis Index

## Analysis Documents

- [Legacy Markdown Analysis](legacy-md-analysis.md)

## Important Legacy Paths

- [`legacy/SPECS.md`](../legacy/SPECS.md) - original full system blueprint.
- [`legacy/API_WEBSOCKET_GUIDE.md`](../legacy/API_WEBSOCKET_GUIDE.md) - root API/WebSocket integration guide.
- [`legacy/DESIGN_BRIEF.md`](../legacy/DESIGN_BRIEF.md) - root SPA UI brief.
- [`legacy/EKSIK.md`](../legacy/EKSIK.md) - root missing-items summary.
- [`legacy/TASKLIST.md`](../legacy/TASKLIST.md) - practical remaining UX/backend TODO list.
- [`legacy/docs/BACKEND_FEATURES.md`](../legacy/docs/BACKEND_FEATURES.md) - backend feature and endpoint inventory.
- [`legacy/src/main.rs`](../legacy/src/main.rs) - legacy route map and service composition.
- [`legacy/migrations/0001_init.sql`](../legacy/migrations/0001_init.sql) - first target for database/domain analysis.
- [`legacy/web/`](../legacy/web/) - legacy browser UI and client helpers.
- [`legacy/UI/`](../legacy/UI/) - additional UI prototypes.

## Recommended Next Analysis Targets

1. Database and domain model: `legacy/migrations/0001_init.sql`, `legacy/dump/`, and model structs.
2. Backend route behavior: `legacy/src/main.rs` plus each handler module.
3. WebSocket/event flow: `legacy/src/websocket/`, `legacy/src/events/`, and `legacy/src/notifications/`.
4. Security-sensitive areas: `legacy/src/auth/`, `legacy/src/permissions/`, `legacy/src/files/`, and config defaults.
5. Frontend behavior: `legacy/web/app.html`, `legacy/web/raw-app.html`, `legacy/web/melt.js`, `legacy/web/connection.js`, and UI prototypes.
