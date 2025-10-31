<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

# Development Setup

## Prerequisites

1. Install [Rust](https://www.rust-lang.org/learn/get-started)
1. Install [just](https://just.systems/man/en/packages.html) (`cargo install just`)
1. Install [Node.js](https://nodejs.org/) (version 20.19.0+ or 22.12.0+)
1. Install [sqlx-cli](https://crates.io/crates/sqlx-cli) (`cargo install sqlx-cli`)
1. Install [pre-commit](https://pre-commit.com) (`apt install pre-commit` or
   `pip install pre-commit`)
1. Run `pre-commit install`

## Setup

1. In the `frontend` directory, run `npm install` to install frontend dependencies.
1. Create a telegram bot by contacting `@BotFather` and issuing `/newbot`.
   ([More details][telegram-bot-tutorial]). Save its token.
1. Create a telegram group for testing, and invite your bot to it. Get the
   group ID (it's a **negative** number) by sending `/help` to that group.
1. Create a file named `.env` with the following contents:

    ```text
    DATABASE_URL=sqlite:dev.db
    RUST_LOG=info,trufotbot=trace
    TELOXIDE_TOKEN=YOUR_BOT_TOKEN_HERE
    TELEGRAM_GROUP_ID=TESTING_GROUP_ID_HERE
    TRUFOTBOT_ALLOWED_USERS=YOUR_USERNAME_HERE
    FRONTEND_URL=http://localhost.localdomain:5173
    ```

1. Run `just reset_db seed` to create `dev.db` with seed data, or `just reset_db`
   for an empty database.

## Running

```shell
just serve_both
```

Or run backend and frontend separately in parallel:

```shell
just serve_backend
just serve_frontend_with_proxy
```

Browse to <http://localhost:5173>

## Documentation

To serve the documentation site locally:

```shell
just docs-serve
```

The site will be available at <http://127.0.0.1:8000>.

[telegram-bot-tutorial]: https://core.telegram.org/bots/tutorial
