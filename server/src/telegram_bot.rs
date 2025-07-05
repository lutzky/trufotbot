use anyhow::{Result, anyhow};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use shared::{
    api::{dose::CreateDose, requests::CreateDoseQueryParams},
    time::now,
};
use teloxide::{
    dptree::deps,
    prelude::*,
    sugar::request::RequestReplyExt,
    types::ReactionType,
    utils::command::{BotCommands, ParseError},
};

use crate::{
    handlers::doses,
    messenger::{callbacks, telegram_sender::TelegramSender},
    models::{Medication, Patient},
    storage::Storage,
};

pub async fn launch(bot: Bot, storage: Storage) {
    let handler: Handler<'static, _, Result<(), _>, _> = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(command_handler),
        )
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler));

    Dispatcher::builder(bot, handler)
        .dependencies(deps![storage.clone()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn command_handler(storage: Storage, bot: Bot, msg: Message, cmd: Command) -> Result<()> {
    match cmd {
        Command::Help => {
            let chat_id = msg.chat.id;
            bot.send_message(
                chat_id,
                format!(
                    r#"<b>TrufotBot help</b>

{}

BTW here's your chat ID, in a separate message so it's easy to copy:"#,
                    Command::descriptions()
                ),
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
            bot.send_message(chat_id, chat_id.to_string()).await?;
        }
        Command::Record(patient_name, medication_name, quantity) => {
            let Some(patient) = Patient::find_by_name(&storage.pool, &patient_name).await? else {
                bot.send_message(
                    msg.chat.id,
                    format!("Error: No such patient {patient_name:?}"),
                )
                .reply_to(msg.id)
                .await?;
                return Ok(());
            };
            let Some(medication) =
                Medication::find_by_name(&storage.pool, &medication_name).await?
            else {
                bot.send_message(
                    msg.chat.id,
                    format!("Error: No such medication {medication_name:?}"),
                )
                .reply_to(msg.id)
                .await?;
                return Ok(());
            };
            doses::record(
                Path((patient.id, medication.id)),
                Query(CreateDoseQueryParams {
                    reminder_message_id: None,
                }),
                State(storage),
                State(TelegramSender::new(bot.clone()).into()),
                Json(CreateDose {
                    quantity,
                    taken_at: now(),
                    noted_by_user: Some(msg.from.unwrap().first_name),
                }),
            )
            .await
            .map_err(|e| {
                log::error!("Error recording dose from button: {e:?}");
                anyhow!("Failed to record dose")
            })?;
            bot.set_message_reaction(msg.chat.id, msg.id)
                .reaction(vec![ReactionType::Emoji {
                    emoji: "👍".to_string(),
                }])
                .await?;
        }
    }
    Ok(())
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    #[command(description = "display this text")]
    Help,
    #[command(
        description = r#"record a dose (e.g. /record Alice "Kids Paracetamol" 2)"#,
        parse_with = parse_record_command
    )]
    Record(String, String, f64),
}

fn parse_record_command(s: String) -> Result<(String, String, f64), ParseError> {
    let Some(sp) = shlex::split(&s) else {
        return Err(ParseError::Custom(anyhow!("shlex failed for {s:?}").into()));
    };
    let [patient_name, medication_name, quantity] = sp.as_slice() else {
        return Err(ParseError::Custom(
            anyhow!("Got {} parameters (want 3)", sp.len()).into(),
        ));
    };
    Ok((
        patient_name.to_string(),
        medication_name.to_string(),
        quantity
            .parse()
            .map_err(|e| ParseError::Custom(anyhow!("invalid quantity: {e}").into()))?,
    ))
}

async fn message_handler(bot: Bot, msg: Message) -> Result<()> {
    bot.send_message(msg.chat.id, "Please see /help".to_string())
        .reply_to(msg.id)
        .await?;
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
        link_callback @ callbacks::Action::Link { .. } => {
            anyhow::bail!("Unexpected callback {link_callback:?}")
        }
    }

    Ok(())
}
