<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

# TrufotBot

TrufotBot is a household-wide medication management system to help families
track and manage medications for all family members and pets! 🏠👨‍👩‍👧‍👦🐕

## The Problem 😓

Most medication tracking apps assume all patients are managed from a single
device - usually one parent's phone. This creates problems:

- The other parent can't log medications
- Caregivers and family members are left out of the loop
- Communication about medication happens in scattered text messages

Therefore, many families opt to just have group chats (often multiple, for
multiple topics) to log medication.

## The Solution 🎯

TrufotBot takes a different approach - it starts with what families already
use: group chats! Here's how it works:

- Create a Telegram group chat for each patient
- TrufotBot joins the chat and sends messages about:
  - Reminders to give medication
  - Notification that medication has been given
- Everyone stays informed through notifications in the chat

We use Telegram because it has an easy-to-use bot API.

## Documentation

For detailed documentation, see the [online docs](https://lutzky.net/trufotbot)
or the [docs/](docs/) directory.
