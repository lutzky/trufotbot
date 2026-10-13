<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

# Environment variables

`TELOXIDE_TOKEN` ***(required)***

:    Your Telegram bot token from @BotFather

`TRUFOTBOT_ALLOWED_USERS` ***(required)***

:    Comma-separated Telegram usernames allowed to interact with the bot

`DATABASE_URL` ***(required)***

:    SQLite database path (e.g. `/db/prod.db`). The database is created if it
     doesn't exist.

`FRONTEND_URL` *(optional, but you almost certainly want to set it)*

:    URL where the web interface is hosted. Used for the links in Telegram
     messages, so it must be reachable from the devices running Telegram. The
     default (`http://0.0.0.0:8080`) is only a placeholder, and doesn't match
     the server's default port (3000). Its hostname should contain a dot (e.g.
     `your-hostname.localdomain`, or an IP address), as Telegram may fail to
     render links to hosts without dots; a warning is logged otherwise.

`RUST_LOG` *(optional)*

:    Logging level (default: `info`)

`TRUFOTBOT_REMINDER_COMPLETION_DELETE_AND_RESEND` *(optional; default `true`)*

:    See [reminders](interacting.md#reminders).

`TRUFOTBOT_SHOW_DOSE_ABSOLUTE_TIME` *(optional; default `false`)*

:    When `true`, [notifications](interacting.md#notifications) include the
     absolute time of the dose in addition to the relative time, e.g. "an hour
     earlier (2026-01-02 (Fri) 08:00)".

`TZ` *(optional)*

:    Timezone for reminders (e.g., `Europe/London`)
