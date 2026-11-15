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
3. The bot will reply with the **group ID** (a negative number like
   `-1001234567890`)
4. Use this group ID when creating a patient (under "Telegram Group ID").

### Reading group messages (for natural input) {: #reading-group-messages }

By default, Telegram bots in groups only receive `/commands`, `@`-mentions, and
replies to the bot's own messages. To use
[natural input](interacting.md#natural-input) (sending messages like
`Nurofen 7.5` without a slash command), the bot must be allowed to read normal
messages in the group.

Do **either** of the following:

1. **Recommended:** Make the bot an **administrator** of the Telegram group.
   This only affects that specific group.
2. Disable privacy mode for the bot as a whole: in **@BotFather**, send
   `/setprivacy`, choose your bot, and select **Disable**. If the bot is
   already in the group, remove and re-invite it so Telegram applies the new
   setting.
