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

# Run e2e tests; use the HEADLESS environment variable to avoid pop-up windows
HEADLESS=true npm run test:e2e

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

use anyhow::Result;
use axum::extract::State;
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
- Use `anyhow::Result` for main functions that don't need specific error types
- Implement `axum::response::IntoResponse` for `ServiceError`
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
}

impl ServiceError {
    pub fn not_found(msg: &str) -> ServiceError {
        ServiceError::NotFound(msg.to_string())
    }
}
```

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

#### Async/Await

- Use `tokio` runtime with `#[tokio::main]`
- Prefer `buffer_unordered` for concurrent operations
- Clone Arc-wrapped values for async tasks

#### Testing

- Place unit tests in same file with `#[cfg(test)]` module
- Use `pretty_assertions` for better diff output
- Use `time::FAKE_TIME` for time-dependent tests

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

## Pre-commit Hooks

The project uses pre-commit hooks (install with `pre-commit install`):

- Trailing whitespace removal
- End-of-file fixer
- YAML validation
- Large file check
- `cargo fmt` check
- `cargo clippy` check
- `cargo check` check
- `cargo test` check (pre-push)
- Spell check with cspell

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

## Environment Variables

Create `.env` file for development:

```text
DATABASE_URL=sqlite:dev.db
RUST_LOG=info,trufotbot=trace
TELOXIDE_TOKEN=YOUR_BOT_TOKEN
TELEGRAM_GROUP_ID=TESTING_GROUP_ID
TRUFOTBOT_ALLOWED_USERS=YOUR_USERNAME
```

## Common Patterns

### State Management

Use Axum's `State` extractor for dependency injection:

```rust
async fn get(
    State(storage): State<Storage>,
    Path(id): Path<i64>,
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
