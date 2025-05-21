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
1. Install just <https://just.systems/man/en/packages.html>
1. Install pre-commit <https://pre-commit.com> (`apt install pre-commit` should do the trick)
1. `pre-commit install`
1. Create a telegram bot by contacting `@BotFather` and issuing `/newbot`. ([More details][telegram-bot-tutorial]). Save its token.
1. Create a telegram group for testing, and invite your bot to it. Get the group ID (it's a **negative** number), e.g. [using a dedicated bot](https://medium.com/@sigmoid90/telegram-tips-get-id-of-your-telegram-group-e063dfc3d52b) (remember to kick the bot out afterwards).
1. Create a file named `.env` with the following contents:

    ```text
    DATABASE_URL=sqlite:dev.db
    RUST_LOG=info,trufotbot=trace
    TELOXIDE_TOKEN=YOUR_BOT_TOKEN_HERE
    TELEGRAM_GROUP_ID=TESTING_GROUP_ID_HERE
    ```

1. Run `just reset_db`. This will create `dev.db` with some seed (dummy) data.
1. Open two terminal windows, and run these two commands in parallel (they will automatically reload on changes to code):
    - `just serve_backend`
    - `just serve_frontend_with_proxy`
1. Browse to <http://localhost:8080>

[telegram-bot-tutorial]: https://core.telegram.org/bots/tutorial

Notes:

- The specific `dev.db` file is already in gitignore for your convenience

### Release

_Under construction_ 🏗️

TODO: Document "release building" by building frontend, copying frontend/dist into server/src/assets, and then building server
