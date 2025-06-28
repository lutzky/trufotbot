use anyhow::{Result, anyhow};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use shared::{
    api::{dose::CreateDose, requests::CreateDoseQueryParams},
    time::now,
};
use teloxide::{dptree::deps, prelude::*};

use crate::{
    handlers::doses,
    messenger::{callbacks, telegram_sender::TelegramSender},
    storage::Storage,
};

pub async fn launch(bot: Bot, storage: Storage) {
    let handler: Handler<'static, _, Result<(), _>, _> = dptree::entry()
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler));

    Dispatcher::builder(bot, handler)
        .dependencies(deps![storage.clone()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn message_handler(bot: Bot, msg: Message) -> Result<()> {
    let chat_id = msg.chat.id;
    bot.send_message(
        chat_id,
        "Here's your chat ID, in a separate message so it's easy to copy:".to_string(),
    )
    .await?;
    bot.send_message(chat_id, chat_id.to_string()).await?;
    Ok(())
}

async fn callback_handler(bot: Bot, q: CallbackQuery, storage: Storage) -> Result<()> {
    let Some(ref data) = q.data else {
        return Ok(());
    };

    let message_id = q.regular_message().map(|message| message.id.0);

    let action: callbacks::Action = serde_json::from_str(data)?;

    match action {
        action @ callbacks::Action::Take {
            patient_id,
            medication_id,
            quantity,
        } => {
            log::debug!("Received callback action: {action:?}");
            doses::record(
                Path((patient_id, medication_id)),
                Query(CreateDoseQueryParams {
                    reminder_message_id: message_id,
                }),
                State(storage),
                State(TelegramSender::new(bot).into()),
                Json(CreateDose {
                    quantity,
                    taken_at: now(),
                    noted_by_user: Some(q.from.first_name),
                }),
            )
            .await
            .map_err(|e| {
                log::error!("Error recording dose from button: {e:?}");
                anyhow!("Failed to record dose")
            })?;
        }
    }

    Ok(())
}
