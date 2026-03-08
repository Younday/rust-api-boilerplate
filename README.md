# Rust API Boilerplate

> A production-ready REST API starter built with Axum and PostgreSQL.

[![CI](https://github.com/Younday/rust-api-boilerplate/actions/workflows/ci.yml/badge.svg)](https://github.com/Younday/rust-api-boilerplate/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## About

A batteries-included Rust API boilerplate you can fork and build on. It provides a clean layered architecture, compile-time checked SQL queries, request validation, structured logging, and a middleware stack ready for production — so you can focus on your domain logic instead of infrastructure.

---

## Features

- **[Axum 0.8](https://github.com/tokio-rs/axum)** — ergonomic async web framework built on Tokio
- **[SQLx 0.8](https://github.com/launchbadge/sqlx)** — async PostgreSQL with compile-time checked queries
- **Layered architecture** — API handlers → Services → Repositories, each with trait abstractions
- **Request validation** — automatic body validation via the `validator` crate and a custom extractor
- **Typed error handling** — `AppError` enum maps cleanly to HTTP responses
- **Tower middleware stack** — rate limiting (5 req/s), 30 s timeouts, CORS, request tracing
- **Structured logging** — `tracing` to stdout in development, rolling daily files in production
- **Graceful shutdown** — handles `SIGTERM` and `Ctrl+C`
- **Cargo workspace** — clean separation into `server`, `database`, and `utils` crates
- **Docker Compose** — one command to start a local Postgres instance
- **Makefile** — short aliases for every common task
- **Pre-commit hooks** — `cargo check` runs before every commit

---

## Quick Start

### Prerequisites

```bash
# Rust stable toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# sqlx-cli (database migrations)
cargo install sqlx-cli --no-default-features --features postgres

# cargo-watch (hot reload)
cargo install cargo-watch

# Docker + Docker Compose (local Postgres)
```

### Run

```bash
# 1. Clone and enter the project
git clone https://github.com/your-username/rust-api-boilerplate.git
cd rust-api-boilerplate

# 2. Configure environment
cp .env.example .env   # edit postgres credentials if needed

# 3. Start Postgres
make db

# 4. Run migrations
make migrate-up

# 5. Start the server (with hot reload)
make start-server
```

Verify it's running:

```bash
curl http://127.0.0.1:8080/api/v1/
# => Server is running!
```

---

## Project Structure

```
rust-api-boilerplate/
├── Cargo.toml              # Workspace root — shared dependency versions
├── Makefile                # Developer commands
├── docker-compose.yaml     # Local Postgres service
├── migrations/             # SQLx migration files (up/down)
└── crates/
    ├── server/             # Axum app — routes, handlers, services, DTOs, extractors
    ├── database/           # SQLx pool, entity models, repository implementations
    └── utils/              # Shared config (AppConfig), error types (AppError)
```

---

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/` | Health check |
| `GET` | `/api/v1/users/` | List all users |
| `POST` | `/api/v1/users/signup` | Create a new user |

---

## Configuration

All configuration is read from environment variables (see `.env`):

| Variable | Default | Description |
|----------|---------|-------------|
| `CARGO_ENV` | `development` | Environment (`development` / `production`) |
| `RUST_LOG` | `debug` | Log level filter |
| `APP_HOST` | `127.0.0.1` | Bind address |
| `APP_PORT` | `8080` | Port |
| `postgres_uri` | `postgresql://...` | Full PostgreSQL connection string |

---

## Development

| Command | Description |
|---------|-------------|
| `make db` | Start Postgres |
| `make db-down` | Stop Postgres |
| `make migrate-up` | Apply all pending migrations |
| `make migrate-down` | Revert last migration |
| `make start-server` | Hot-reload dev server |
| `cargo test --workspace` | Run all tests |
| `cargo fmt --all` | Format code |
| `cargo clippy --workspace -- -D warnings` | Lint |
| `cargo sqlx prepare --workspace` | Regenerate SQLx offline cache (required before CI) |

---

## Contributing

See [CLAUDE.md](CLAUDE.md) for architecture conventions, the layer model, instructions for adding new resources, and the testing strategy.
