<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

# Telegram setup

These are abbreviated instructions from <https://core.telegram.org/bots/tutorial>.

!!! tip

    Use <https://web.telegram.org> for these steps instead of your phone, for
    easier copying and pasting.

## Creating a Telegram Bot {: #create-bot }

1. Open Telegram and search for **@BotFather**
2. Send `/newbot`
3. Follow the prompts to name your bot
4. Copy the bot token - you'll need it for `TELOXIDE_TOKEN` in the
   `secrets.env` file

## Configuring your bot for a telegram group {: #group-setup }

TrufotBot will send announcements on telegram groups; these can be configured
per-patient (but the same group can be reused for multiple patients). For
instance, you can have a group "Medication for Alice", which will have everyone
who gives Alice medication.

1. Invite your bot to a telegram group
2. Send `/help` to the group
3. The bot will reply with the **group ID** (a negative number like `-1001234567890`)
4. Use this group ID when creating a patient (under "Telegram Group ID).
