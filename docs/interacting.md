<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

# Interacting with the Bot

TrufotBot can be controlled via commands in any Telegram chat where it's
present. You can use it in private chats, group chats, or through inline
autocomplete from anywhere.

## Commands

### `/help`

Displays help information and available commands. The bot will also provide a
copyable button containing the current chat's ID.

### `/record`

Records a medication dose. The format is:

```text
/record <patient> <medication> <quantity> [by <username>]
```

**Parameters:**

`patient` ***(required)***

:   Name of the patient taking the medication

`medication` ***(required)***

:   Name of the medication

`quantity` ***(required)***

:   Dosage given

`username` *(optional)*

:   Name of the person giving the medication (defaults to the first name of who
    sent the message)

**Examples:**

```text
/record Alice "Kids Paracetamol" 2
/record Bob Aspirin 1 by Sarah
/record "Fido" "Heartworm Pill" 1
```

If the patient or medication name contains spaces, use quotes around the name.

## Using Auto-Completion

The easiest way to record a dose is through inline autocomplete. Type your bot
username (e.g., `@YourMedBot`) in any Telegram chat and start typing:

```text
@YourMedBot ali par
```

The bot will suggest completions based on your history. Selecting a suggestion
inserts the full `/record` command:

```text
/record Alice Paracetamol 1
```

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

## Notification Format

When a dose is recorded, the bot posts a message like:

> Alice took Paracetamol (2) 5 minutes ago

Buttons appear below the message:

- **Edit... ✏️** - Opens the web interface to edit the dose
- **Repeat 🔁** - Records the same dose again

## Keyboard Shortcuts

Telegram allows you to pin bots for quick access:

1. Open a chat with your bot
2. Tap the bot name at the top
3. Tap the pin icon to add it to your shortcuts
4. Now you can quickly access it from any chat via the attachment menu
