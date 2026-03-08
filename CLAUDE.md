# CLAUDE.md — Rust API Boilerplate

This file guides AI assistants and contributors working on this project. It covers architecture, conventions, commands, and how to extend the codebase.

---

## Project Overview

A production-ready Rust REST API boilerplate using **Axum** and **PostgreSQL** (via SQLx), structured as a Cargo workspace. Fork or clone this to bootstrap a new API project.

**Tech stack:**
- **Axum 0.8** — async web framework
- **SQLx 0.8** — async PostgreSQL driver with compile-time checked queries
- **Tokio** — async runtime
- **Tower** — middleware (rate limiting, timeouts, CORS, tracing)
- **clap + dotenvy** — config via env vars and CLI args
- **validator** — request body validation
- **thiserror** — typed error handling
- **tracing** — structured logging (stdout in dev, rolling files in prod)

---

## Quick Start

### Prerequisites

```bash
# Rust (stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# sqlx-cli (for migrations)
cargo install sqlx-cli --no-default-features --features postgres

# cargo-watch (for hot reload)
cargo install cargo-watch

# Docker + Docker Compose (for local Postgres)
```

### Run locally

```bash
# 1. Copy and configure environment
cp .env.example .env   # edit postgres_uri if needed

# 2. Start Postgres
make db

# 3. Run migrations
make migrate-up

# 4. Start the server with hot reload
make start-server
# or: cargo run -p server
```

The API will be available at `http://127.0.0.1:8080`.

---

## Workspace Structure

```
rust-api-boilerplate/
├── Cargo.toml              # Workspace root — shared dependency versions
├── Cargo.lock
├── Makefile                # Developer commands (see below)
├── rustfmt.toml            # Formatting rules
├── .pre-commit-config.yaml # Pre-commit hooks (cargo check)
├── .env                    # Local env vars (never commit secrets)
├── docker-compose.yaml     # Local Postgres service
├── migrations/             # SQLx migration files (up/down)
└── crates/
    ├── server/             # Axum app: routes, handlers, DTOs, services, extractors
    │   └── src/
    │       ├── main.rs
    │       ├── app.rs          # TCP listener, state init, graceful shutdown
    │       ├── router.rs       # Route tree + middleware stack
    │       ├── logger.rs       # Tracing setup
    │       ├── api/            # HTTP handlers (thin — delegate to services)
    │       ├── services/       # Business logic (trait + impl pattern)
    │       ├── dtos/           # Request/response structs with validation
    │       └── extractors/     # Custom Axum extractors (e.g. ValidationExtractor)
    ├── database/           # SQLx pool, models, repositories
    │   └── src/
    │       ├── lib.rs          # Database struct + pool init
    │       └── user/
    │           ├── model.rs        # User entity
    │           └── repository.rs   # SQL queries + UserRepositoryTrait
    └── utils/              # Shared utilities used by all crates
        └── src/
            ├── config.rs       # AppConfig (parsed from env/CLI via clap)
            └── errors.rs       # AppError enum + IntoResponse impl
```

---

## Key Commands

| Command | Description |
|---|---|
| `make db` | Start Postgres via Docker Compose |
| `make db-down` | Stop Postgres |
| `make migrate-up` | Run all pending migrations |
| `make migrate-down` | Revert last migration |
| `make start-server` | Start server with hot reload (cargo-watch) |
| `cargo test --workspace` | Run all tests |
| `cargo fmt --all` | Format all code |
| `cargo clippy --workspace -- -D warnings` | Lint (treat warnings as errors) |
| `cargo sqlx prepare --workspace` | Regenerate `.sqlx/` offline query cache |
| `cargo build --release -p server` | Production build |

---

## Architecture & Conventions

### Rust Conventions

- Use `thiserror` for library errors, `anyhow` only in binary crates or tests
- No `.unwrap()` or `.expect()` in production code — propagate errors with `?`
- Prefer `&str` over `String` in function parameters; return `String` when ownership transfers
- Use `clippy` with `#![deny(clippy::all, clippy::pedantic)]` — fix all warnings
- Derive `Debug` on all public types; derive `Clone`, `PartialEq` only when needed
- No `unsafe` blocks unless justified with a `// SAFETY:` comment

### Database

- All queries use SQLx `query!` or `query_as!` macros — compile-time verified against the schema
- Migrations in `migrations/` using `sqlx migrate` — never alter the database directly
- Use `sqlx::Pool<Postgres>` as shared state — never create connections per request
- All queries use parameterized placeholders (`$1`, `$2`) — never string formatting

```rust
// BAD: String interpolation (SQL injection risk)
let q = format!("SELECT * FROM users WHERE id = '{}'", id);

// GOOD: Parameterized query, compile-time checked
let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id)
    .fetch_optional(&pool)
    .await?;
```

### Layer model

```
HTTP Request
    └── Router (router.rs)
        └── Handler (api/*.rs)          — parse request, call service, return response
            └── Service (services/*.rs) — business logic, calls repository
                └── Repository (database/src/*/repository.rs) — SQL queries
```

**Key rules:**
- Handlers must be thin. All logic lives in services.
- Services depend on repository **traits**, not concrete types — enables mocking in tests.
- Services are injected into Axum state as `Arc<dyn ServiceTrait + Send + Sync>`.

### Adding a new resource (e.g. `Post`)

1. **Migration** — add `migrations/<timestamp>_post.up.sql` and `.down.sql`
2. **Model** — `crates/database/src/post/model.rs` with a `Post` struct (derive `FromRow`, `Serialize`)
3. **Repository** — `crates/database/src/post/repository.rs` with a `PostRepositoryTrait` and `impl PostRepositoryTrait for Database`
4. **Service** — `crates/server/src/services/post.rs` with a `PostServiceTrait` and impl
5. **DTOs** — `crates/server/src/dtos/post_dto.rs` with validated request structs
6. **Handler** — `crates/server/src/api/posts.rs`
7. **Router** — add routes in `crates/server/src/router.rs` under `/api/v1/posts`

### Error handling

- All errors must map to `AppError` (`crates/utils/src/errors.rs`).
- Use `?` to propagate errors. Never `.unwrap()` outside of tests.
- `AppError` implements `IntoResponse` — handlers return `Result<impl IntoResponse, AppError>`.

### Validation

- Use `ValidationExtractor<T>` instead of plain `Json<T>` for request bodies that need validation.
- Add `#[validate(...)]` attributes to DTO fields (see `crates/server/src/dtos/user_dto.rs`).

### Configuration

- All config values come from env vars or CLI args, parsed by `AppConfig` (`crates/utils/src/config.rs`).
- Never hardcode hostnames, ports, or credentials.
- Access config via the Axum `State` extractor.

---

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `CARGO_ENV` | `development` | Environment (`development` / `production`) |
| `RUST_LOG` | `debug` | Log level filter |
| `APP_HOST` | `127.0.0.1` | Server bind address |
| `APP_PORT` | `8080` | Server port |
| `POSTGRES_HOST` | `localhost` | Postgres host |
| `POSTGRES_PORT` | `5432` | Postgres port |
| `POSTGRES_USER` | `admin` | Postgres user |
| `POSTGRES_PASSWORD` | `admin` | Postgres password |
| `POSTGRES_DB` | `rust` | Postgres database name |
| `postgres_uri` | (constructed) | Full connection string |

Copy `.env` to `.env.example` (with placeholders) before committing — never commit real credentials.

---

## Testing

### Database layer

Tests use `#[sqlx::test]` which spins up a temporary database per test:

```rust
// crates/database/tests/user.rs
#[sqlx::test]
async fn create_user(pool: PgPool) -> sqlx::Result<()> { ... }
```

Requires a running Postgres. Set `DATABASE_URL` in your env.

### API / integration layer

Add tests to `crates/server/tests/api.rs`. Use `axum_test` or build an `axum::Router` directly and call it via `tower::ServiceExt::oneshot`.

- Unit tests in `#[cfg(test)]` modules within each source file
- Integration tests in `tests/` directory using a real PostgreSQL (Testcontainers or Docker)
- Use `#[sqlx::test]` for database tests with automatic migration and rollback
- Mock external services with `mockall` or `wiremock`

## Code Style

- Max line length: 100 characters (enforced by rustfmt)
- Group imports: `std`, external crates, `crate`/`super` — separated by blank lines
- Modules: one file per module, `mod.rs` only for re-exports
- Types: PascalCase, functions/variables: snake_case, constants: UPPER_SNAKE_CASE


### Run all tests

```bash
cargo test --workspace
```

### SQLx offline mode (required for CI)

Before pushing changes that modify SQL queries, regenerate the offline query cache:

```bash
cargo sqlx prepare --workspace
```

This creates/updates `.sqlx/` files. Commit them. CI runs with `SQLX_OFFLINE=true` so it never needs a live database.

---

## CI/CD (GitHub Actions)

Workflows live in `.github/workflows/`.

### `ci.yml` — runs on every push and PR

1. **Lint** — `cargo fmt --check --all` + `cargo clippy --workspace -- -D warnings`
2. **Build** — `cargo build --workspace`
3. **Test** — `cargo test --workspace` with `SQLX_OFFLINE=true`
4. **Docker build** — build the production image to catch Dockerfile errors early

### `release.yml` — runs on tags (`v*`)

1. Build release binary
2. Build and push Docker image to registry (e.g. GitHub Container Registry)
3. Optional: deploy to staging/production

---

## Docker & Deployment

### Local development

`docker-compose.yaml` provides Postgres only. The app runs on your host.

```bash
make db          # start postgres
make start-server  # run app locally
```

### Production

A multi-stage `Dockerfile` should be used:

```dockerfile
# Stage 1: build
FROM rust:1.83-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p server

# Stage 2: runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/server /usr/local/bin/server
EXPOSE 8080
CMD ["server"]
```

All configuration is injected at runtime via environment variables — no secrets baked into the image.

For local full-stack Docker (app + db), add an `app` service to `docker-compose.yaml` that builds from the `Dockerfile`.

---

## Code Style

- Formatting is enforced by `rustfmt.toml`. Run `cargo fmt --all` before committing.
- The pre-commit hook (`cargo check`) catches compilation errors before commits.
- Naming: `snake_case` for modules/functions/variables, `PascalCase` for types/traits.
- Prefer `?` over `unwrap()`/`expect()` in non-test code.
- Keep handlers thin — extract all logic into the service layer.
- Use `Arc<dyn Trait + Send + Sync>` for injectable dependencies in Axum state.

---

## Not Yet Implemented

The following are common production needs that are not in the boilerplate yet. Add them as needed:

- **Authentication** — JWT middleware (`tower` layer or Axum `middleware::from_fn`), password hashing (`argon2` or `bcrypt`)
- **Database migrations with data** — the migration files in `migrations/` are empty stubs; fill them in
- **OpenAPI docs** — `utoipa` crate integrates with Axum for Swagger UI
- **Health check details** — extend `GET /api/v1/` to include DB connectivity status
- **Rate limiting per user/IP** — current rate limiter is global; use `governor` for per-key limiting
- **Secrets management** — use environment injection from a secrets manager (Vault, AWS SSM, etc.) rather than `.env` files in production
