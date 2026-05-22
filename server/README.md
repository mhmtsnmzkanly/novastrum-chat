# Novastrum Server

Rust + Axum backend skeleton for the Novastrum rebuild.

Common commands:

```sh
cargo fmt
cargo check
cargo test
cargo run
```

Migrations live in `server/migrations/` and are run manually with `sqlx-cli`:

```sh
export DATABASE_URL='mysql://novastrum_user:password@127.0.0.1:3306/novastrum'
sqlx migrate info
sqlx migrate run
```

The server does not auto-run migrations at startup. See `../docs/development.md` for the full local development guide.
