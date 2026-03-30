<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

# Interacting with the Bot

In addition the the web UI, much of the interaction with TrufotBot is through,
unsurprisingly, a bot (on Telegram).

TrufotBot uses Telegram's [Inline Keyboards] to make some interactions quicker,
see below.

## Notifications

When medication is given, a message will be sent in the configured Telegram
group. It will include the time of the dose relative to the time the
notification was sent (usually "now", but can be e.g. "5 minutes earlier" after
editing).

Notifications will have these inline keyboard buttons:

- **Edit... ✏️** - This allows you to retroactively modify the time, quantity,
  and person who gave the dose. This also allows you to delete a dose.
- **Repeat 🔁** - This immediately records a new identical dose to the selected
  one (except for the time, which will be "now").

!!! note

      Editing a dose also edits the telegram message. The Telegram app doesn't
      send notifications for edits, so this is effectively silent. It may take
      some time to propagate to other users.

## Reminders

Reminders are sent in the configured Telegram group. They have these inline
keyboard buttons; clicking through any of them will remove the reminder and
send a [notification](#notifications) instead.

!!! note

      Because the reminder is removed and a notification is sent instead:

      - Telegram will notify other members of the group (like for any
        notification)
      - The message will be a new one (even if there were other messages after
        the reminder).

      This prevents a delay, as Telegram doesn't immediately distribute edits
      to other group members; this helps avoid situations where the other
      medication-giver in the group doesn't realize medication has been given.

      To change this behavior, set the environment variable
      `TRUFOTBOT_REMINDER_COMPLETION_DELETE_AND_RESEND` to `false`.

- Take 5 💊 - This marks the reminder as completed. Note that 5 will be
  replaced by the quantity of the most recent non-zero dose for that medication
  and patient.
- Skip ⏭️ - This is equivalent to "Take 0", and explicitly marks a dose as skipped.
- Take... 📝 - This opens a browser window to
  [`FRONTEND_URL`](environment-variables.md) for recording a specific dosage,
  with a specific time, given by a specific person.

## Commands

These commands can be sent directly to the bot in private messages or in any
group the bot is invited to.

### `/help`

Displays help information and available commands. The bot will also provide a
copyable button containing the current chat's ID.

### `/record`

Records a medication dose. The format is:

      /record <patient> <medication> <quantity> [by <username>]

**Examples:**

      /record Alice "Kids Paracetamol" 2
      /record Bob Aspirin 1 by Sarah
      /record "Fido" "Heartworm Pill" 1

If the patient or medication name contains spaces, use quotes around the name.

!!! tip

    Patient and medication have to be spelled precisely; therefore, it's
    recommended to use auto-completion.

## Using Auto-Completion

The easiest way to record a dose is through inline autocomplete. Type your bot
username (e.g., `@YourMedBot`) in any Telegram chat and start typing:

      @YourMedBot ali par

The bot will suggest completions based on your history. Selecting a suggestion
inserts the full `/record` command:

      /record Alice Paracetamol 1

### How It Works

1. Type `@YourMedBot` followed by a search term
2. TrufotBot shows up to 10 suggestions based on:
      - Matching patient names
      - Matching medication names
      - Your recent dose history
3. Tap a suggestion to insert it as a message
4. Send the message to record the dose

### Examples

If you frequently give Alice Paracetamol, typing partial matches works:

| You Type | Suggested Completion |
| -------- | -------------------- |
| `@YourMedBot al p` | `/record Alice Paracetamol 1` |
| `@YourMedBot alice 2` | `/record Alice Paracetamol 2` |
| `@YourMedBot bob` | `/record Bob Metformin 1` |
