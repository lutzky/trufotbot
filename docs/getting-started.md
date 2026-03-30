<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

# Getting started

This guide covers running TrufotBot using Docker, ideal for users who want to
self-host without developing the application.

## Prerequisites

- [Docker](https://docs.docker.com/get-docker/)
- [Telegram](https://telegram.org/) (e.g. on your phone)
    - You will also need to follow some [telegram setup steps](telegram-setup.md).

## Docker configuration

Create a directory, and in it a `docker-compose.yaml` file:

```yaml
services:
  trufotbot:
    image: ghcr.io/lutzky/trufotbot:latest
    container_name: trufotbot
    restart: unless-stopped
    env_file:
      - secrets.env
    ports:
      - "3000:3000"
    volumes:
      - ./db:/db
    environment:
      - DATABASE_URL=/db/prod.db
      - FRONTEND_URL=http://your-hostname.localdomain:3000
      - RUST_LOG=info
      - TZ=Europe/London
```

[Create a telegram bot](telegram-setup.md#create-bot). Then, Create a file called
`secrets.env` (referenced above using `env_file`) with your settings:

```text
TELOXIDE_TOKEN=<YOUR_TELEGRAM_BOT_TOKEN>
TRUFOTBOT_ALLOWED_USERS=your_telegram_username
```

See also [environment variables](environment-variables.md).

## Running with Docker

### Starting the Application

```shell
mkdir -p db
docker compose up -d
```

The web interface will be available at `http://localhost:3000`.

## Next Steps

Once running, see [Interacting with the Bot](interacting.md) to learn how to
record medications and use the bot.
