<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

# Environment variables

`TELOXIDE_TOKEN` ***(required)***

:    Your Telegram bot token from @BotFather

`TRUFOTBOT_ALLOWED_USERS` ***(required)***

:    Comma-separated Telegram usernames allowed to interact with the bot

`DATABASE_URL` *(optional)*

:    SQLite database path (default: `/db/prod.db`)

`FRONTEND_URL` *(optional)*

:    URL where the web interface is hosted

`RUST_LOG` *(optional)*

:    Logging level (default: `info`)

`TZ` *(optional)*

:    Timezone for reminders (e.g., `Europe/London`)
