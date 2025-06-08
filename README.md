# TrufotBot

TrufotBot is a household-wide medication management system to help  families track and manage medications for all family members and pets! 🏠👨‍👩‍👧‍👦🐕

## The Problem 😓

Most medication tracking apps assume all patients are managed from a single device - usually one parent's phone. This creates problems:

- The other parent can't log medications
- Caregivers and family members are left out of the loop
- Communication about medication happens in scattered text messages

Therefore, many families opt to just have group chats (often multiple, for multiple topics) to log medication.

## The Solution 🎯

TrufotBot takes a different approach - it starts with what families already use: group chats! Here's how it works:

- Create a Telegram group chat for each patient
- TrufotBot joins the chat and sends messages about:
  - Reminders to give medication
  - Notification that medication has been given
- Everyone stays informed through notifications in the chat

We use Telegram because it has an easy-to-use bot API.

## Running TrufotBot 🚧

### Development

1. Install rust <https://www.rust-lang.org/learn/get-started>
1. Install just <https://just.systems/man/en/packages.html> (`cargo install just`)
1. Run `rustup target add wasm32-unknown-unknown`
1. Install trunk <https://trunkrs.dev/> (`cargo install trunk`)
1. Install sqlx-cli <https://crates.io/crates/sqlx-cli> (`cargo install sqlx-cli`)
1. Install pre-commit <https://pre-commit.com> (`apt install pre-commit` should do the trick)
1. Run `pre-commit install`
1. Create a telegram bot by contacting `@BotFather` and issuing `/newbot`. ([More details][telegram-bot-tutorial]). Save its token.
1. Create a telegram group for testing, and invite your bot to it. Get the group ID (it's a **negative** number) by sending `/group_id@YOUR-BOT-NAME` to that group.
1. Create a file named `.env` with the following contents:

    ```text
    DATABASE_URL=sqlite:dev.db
    RUST_LOG=info,trufotbot=trace
    TELOXIDE_TOKEN=YOUR_BOT_TOKEN_HERE
    TELEGRAM_GROUP_ID=TESTING_GROUP_ID_HERE
    ```

1. Run `just reset_db` or `just reset_db seed` - both will create `dev.db`, the latter with some seed (dummy) data.
1. Run `just serve_both` (or, if that doesn't work, run `just serve_backend` and `just serve_frontend_with_proxy` in parallel)
1. Browse to <http://localhost:8080>

[telegram-bot-tutorial]: https://core.telegram.org/bots/tutorial

Notes:

- The specific `dev.db` file is already in gitignore for your convenience

## Release building

### Direct

```shell
just release_frontend
cargo build --release --bin trufotbot
```

Your output binary is now in `target/release/trufotbot`. It's a self-contained binary.

### Docker

Docker builds are entirely containerized, meaning that your local build system should not affect them. They take longer as a result, but some caching is performed for subsequent builds.

```shell
docker build . -t trufotbot:dev
```

Example `docker-compose.yaml` for this:

```yaml
services:
  trufotbot-dev:
    container_name: trufotbot-dev
    restart: unless-stopped
    image: trufotbot:dev
    env_file: "secrets.env" # Include TELOXIDE_TOKEN here
    ports:
      - 3000:3000
    volumes:
      - ./db:/db
    environment:
      - DATABASE_URL=/db/prod.db
      - RUST_LOG=info,trufotbot=trace
      - FRONTEND_URL=http://your-hostname.localdomain:3000
      - TZ=Europe/Dublin
```
