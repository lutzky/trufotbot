<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

# AGENTS.md - TrufotBot Developer Guide

This document provides guidelines for agents working on the TrufotBot codebase.

## Project Overview

TrufotBot is a household medication management system using Telegram for notifications. It's a full-stack application with:

- **Backend**: Rust with Axum web framework, SQLite database, Telegram bot integration
- **Frontend**: Vue.js 3 with TypeScript

## Build Commands

### Backend (Rust)

```bash
# Run all tests
cargo test

# Run a specific test (use test name pattern)
cargo test <test_name_pattern>

# Run tests in watch mode (reruns on file changes)
cargo watch -cx "test <target>"

# Using just (recommended)
just test                    # Run all tests
just test target='patients'  # Run tests matching 'patients'

# Format code
cargo fmt

# Lint code (runs clippy)
cargo clippy --all-targets --all-features -- -D warnings

# Check for errors (faster than build)
cargo check --all-targets

# Build for release
cargo build --release --bin trufotbot

# Reset database
just reset_db        # Create empty dev.db
just reset_db seed   # Create dev.db with seed data
```

### Frontend (Vue.js)

```bash
cd frontend

# Run unit tests
npm run test:unit

# Run a single test file
npm run test:unit -- --run <path>

# Run e2e tests (recommended; see "Frontend e2e tests" below for why the flags matter)
just frontend-e2e
just frontend-e2e e2e/some.spec.ts --project=chromium   # one file, one browser

# Lint and fix
npm run lint

# Format code
npm run format

# Type check
npm run type-check

# Combined check (lint + type-check + unit tests)
just frontend-check

# Build frontend
just release_frontend
```

### Documentation

```bash
just docs-serve   # Serve the mkdocs site locally at http://127.0.0.1:8000
just docs-build   # Build the site
```

User-facing documentation lives in `docs/`; update it (e.g.
`docs/environment-variables.md`) when changing user-visible behavior or
configuration.

### Full Stack

```bash
# Serve both frontend and backend with hot reload
just serve_both

# Serve backend only
just serve_backend

# Serve frontend with proxy to backend
just serve_frontend_with_proxy
```

## Code Style Guidelines

### Rust Backend

#### Formatting

- Run `cargo fmt` before committing
- Uses standard Rust formatting (4 spaces, snake_case)
- Maximum line length: 100 characters (default)

#### Imports

Group imports in this order:

1. Standard library (`std`, `core`)
2. External crates (alphabetically)
3. Local modules (`crate::`)

```rust
use std::{str::FromStr, sync::Arc};

use axum::extract::State;
use color_eyre::eyre::Result;
use serde::Serialize;

mod api;
mod models;
```

#### Naming Conventions

- **Types**: PascalCase (`Patient`, `ServiceError`)
- **Functions/variables**: snake_case (`get_patient`, `storage_pool`)
- **Constants**: UPPER_SNAKE_CASE
- **Traits**: PascalCase with `-er` suffix when appropriate

#### Error Handling

- Use `thiserror` for application errors with `ServiceError` enum
- Use `color_eyre::eyre::Result` for main functions that don't need specific
  error types
- Implement `axum::response::IntoResponse` for `ServiceError`
- Use the `ServiceError::not_found` and `ServiceError::bad_request` constructors;
  they accept anything that is `Into<String>`
- Log errors before returning

```rust
#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Database Error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("{0} not found")]
    NotFound(String),
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Internal server error")]
    InternalError(#[from] eyre::Error),
}

impl ServiceError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
}
```

#### ID Types

- Use the newtypes from `src/ids.rs` (`PatientId`, `MedicationId`, `DoseId`,
  `MessageId`) instead of bare integers for IDs, including in function
  parameters, API path/query parameters, and test helpers
- Define new ID types with the `define_id!` macro in `src/ids.rs`
- Telegram group IDs use `TelegramGroupId` (`i64`), which is deliberately *not*
  made with `define_id!`: it is a decimal **string** in JSON (numbers are
  rejected), so JavaScript clients can't silently lose precision above 2^53.
  It stays an integer in SQLite. Use a type override in `sqlx::query!` /
  `query_as!` (`telegram_group_id AS "telegram_group_id: TelegramGroupId"`).
  In the frontend it is a `string`; never convert it to a `number`
- Messaging code (`Messenger`, `Sender`) uses teloxide's `ChatId`; everything
  DB- or API-facing uses `TelegramGroupId`. Convert with `.into()` where a DB
  value enters the messaging code, not inside `Messenger`'s API
- utoipa only collects schemas referenced from request/response bodies. An ID
  type used only as a path/query parameter must also be listed in the
  `components(schemas(...))` of `ApiDoc` in `src/main.rs`

#### Database (sqlx)

- Use `sqlx::query!` for queries that return rows
- Use `sqlx::query_as!` for queries returning typed results
- Use `sqlx::test` macro for integration tests with fixtures
- Place fixtures in `src/fixtures/` directory

```rust
#[sqlx::test(fixtures("../fixtures/patients.sql"))]
async fn list_patients_correct(db: SqlitePool) {
    // test code
}
```

#### API Documentation

- Use `utoipa` macros for OpenAPI documentation
- Add `#[utoipa::path(...)]` to all API handlers
- Define constants for UTOIPA_TAG per module

```rust
pub const UTOIPA_TAG: &str = "patients";

#[utoipa::path(
    get,
    path = "/api/patients/{id}",
    summary = "Get a patient",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, body = Patient),
        (status = 404, description = "Patient not found"),
    ),
)]
```

#### Time

- Never call `chrono::Local::now()` or use `chrono::Local` directly (clippy
  denies this via `.clippy.toml`)
- Use `time::now()` instead of `Utc::now()`, and `time::local_timezone()`
  instead of `chrono::Local`, so time can be mocked in tests
- Use the `DateTimeExt` methods `local_display()` and `relative_to()` for
  formatting timestamps

#### Async/Await

- Use `tokio` runtime with `#[tokio::main]`
- Prefer `buffer_unordered` for concurrent operations
- Clone Arc-wrapped values for async tasks

#### Testing

- Place unit tests in same file with `#[cfg(test)]` module
- Use `pretty_assertions` for better diff output
- Use `time::FAKE_TIME` for time-dependent tests. `time::now()` panics in tests
  unless `FAKE_TIME` is set, so wrap the test body in
  `FAKE_TIME.scope("2025-01-02T00:00:00Z", async { ... }).await` (or
  `FAKE_TIME.sync_scope` for non-async code). Tests run with `UTC` as the local
  timezone.

### Vue.js Frontend

#### CSS/HTML Framework

- Adhere to PicoCSS best-practices
- PicoCSS is a classless framework that styles semantic HTML elements directly
- Use native HTML5 semantic elements (`<header>`, `<main>`, `<article>`, `<section>`, `<nav>`, `<footer>`, `<form>`, `<table>`, etc.)
- Avoid unnecessary `<div>` wrappers; prefer semantic elements
- Forms, tables, and lists are styled automatically - use appropriate tags
- Use `.container` class for centered content layout
- Use `.grid` class for responsive grid layouts
- Dark mode is automatic based on user preference (uses `prefers-color-scheme`)
- Customize via CSS variables (see PicoCSS docs) if theming is needed
- Minimize custom CSS classes - let PicoCSS style native elements
- Avoid utility-class approaches (like Tailwind); prefer semantic HTML structure

#### Formatting for vue

- Uses Prettier (run `npm run format`)
- 2 spaces indentation

#### TypeScript

- Strict mode enabled
- Use TypeScript types, avoid `any`

#### Components

- Vue 3 Composition API with `<script setup>`
- Use Vue Router for navigation

#### Frontend e2e tests

Playwright tests live in `frontend/e2e/` and run against a Vite dev server that Playwright starts
itself, in both chromium and firefox. They do not need the Rust backend.

The tests are hermetic: whatever else is running (`npm run dev`, `just serve_both`, a real backend on
port 3000) cannot affect them.

- The e2e server uses its own port (5199, or `E2E_PORT`; 4173 with the preview server on CI) with
  `--strictPort`, and an existing server is never reused. If something already occupies that port,
  Playwright fails with "already used" rather than testing someone else's code.
- The dev server's `/api` proxy (normally `http://localhost:3000`, overridable via
  `API_PROXY_TARGET`) is pointed at a dead address for e2e. All API calls must be mocked with
  `page.route`; one that slips through gets a connection-refused 500 instead of reaching a backend.

- Import `test` and `expect` from `./fixtures`, not from `@playwright/test`. The fixture returns a
  500 for any unmocked `/api/**` request and mocks `/api/status`.
- Mock each endpoint the page uses with `page.route(...)`. Keep state in the test (e.g. a `store`
  object) when a test needs to save and then reload, and make the mock reject the same inputs the
  real server would, so type mismatches like `""` for an `i64` are caught.
- Use `just frontend-e2e` (or `just frontend-e2e <args>`) rather than calling Playwright directly.
  It sets:
  - `HEADLESS=true`: no pop-up browser windows.
  - `TZ=UTC`: in sandboxes/containers without a timezone, the browser reports `Etc/Unknown`,
    `DaylightSavingsWarning` throws while mounting, and pages stay stuck on their loading state.
  - `PLAYWRIGHT_HTML_OPEN=never`: otherwise the default `html` reporter serves the report after a
    failure and the command never exits.
- A fresh checkout has no `node_modules`; run `npm ci` in `frontend/` first.
- Before trusting a new regression test, temporarily revert the fix and check that the test fails.

## Pre-commit Hooks

The project uses pre-commit hooks (install with `pre-commit install`):

- Trailing whitespace removal
- End-of-file fixer
- YAML validation
- Large file check
- Spell check with cspell (add new words to `project-words.txt`)
- Markdown lint for `docs/` and `README.md` (markdownlint-cli2)
- No committed `axum::debug_handler`
- `cargo fmt` check
- Frontend format check (prettier)
- `cargo clippy` check
- `cargo check` check
- `cargo test` check (pre-push)
- `just api-update` check, which verifies the generated OpenAPI schema and
  TypeScript client are up to date

Run all checks manually:

```bash
pre-commit run --all-files
```

## API Regeneration

When backend API changes, regenerate the frontend client:

```bash
just api-update
```

This generates `frontend/trufotbot-openapi.json` and TypeScript client code.
See also [ID Types](#id-types) for a utoipa caveat with parameter-only schemas.

## Environment Setup

`.env` and `dev.db` are gitignored, so they are missing in a fresh checkout or
new worktree. Run this first, before building or running pre-commit hooks (the
sqlx macros need `DATABASE_URL` to compile):

```bash
just reset_db
```

This creates an empty `dev.db` and, if `.env` doesn't exist, a base `.env` with
`DATABASE_URL` and `RUST_LOG`. Do not create `.env` by hand.
This is enough for development and tests; Telegram settings (`TELOXIDE_TOKEN`,
`TELEGRAM_GROUP_ID`, `TRUFOTBOT_ALLOWED_USERS`) are only needed to talk to a
real bot, and are added to `.env` by a human.

## Common Patterns

### State Management

Use Axum's `State` extractor for dependency injection:

```rust
async fn get(
    State(storage): State<Storage>,
    Path(id): Path<PatientId>,
) -> Result<Json<Patient>, ServiceError> {
    Patient::get(&storage.pool, id).await
}
```

### Database Transactions

```rust
let mut tx = pool.begin().await?;
// ... operations
tx.commit().await?;
```

### Clone for Async

```rust
let storage = storage.clone();
tokio::spawn(async move {
    // use storage here
});
```
