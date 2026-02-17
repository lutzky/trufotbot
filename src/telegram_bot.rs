use std::sync::Arc;

use crate::{
    api::{dose::CreateDose, requests::CreateDoseQueryParams},
    app_state::Config,
    time::now,
};
use anyhow::{Result, anyhow};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use teloxide::{
    dptree::deps,
    prelude::*,
    sugar::request::RequestReplyExt,
    types::{
        CopyTextButton, InlineKeyboardButton, InlineKeyboardMarkup, InlineQueryResult,
        InlineQueryResultArticle, InputMessageContent, InputMessageContentText, ReactionType,
    },
    utils::command::{BotCommands, ParseError},
};

use crate::{
    autocomplete,
    handlers::doses,
    messenger::{callbacks, telegram_sender::TelegramSender},
    models::{Medication, Patient},
    storage::Storage,
};

pub async fn launch(bot: Bot, storage: Storage, config: Arc<Config>) {
    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(command_handler),
        )
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler))
        .branch(Update::filter_inline_query().endpoint(inline_query_handler));

    Dispatcher::builder(bot, handler)
        .dependencies(deps![storage.clone(), config.clone()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn command_handler(
    storage: Storage,
    config: Arc<Config>,
    bot: Bot,
    msg: Message,
    cmd: Command,
) -> Result<()> {
    check_user(
        msg.from
            .as_ref()
            .map(|u| &u.username)
            .unwrap_or(&None)
            .as_deref(),
    )?;

    match cmd {
        Command::Help => {
            let chat_id = msg.chat.id;

            let copy_button = InlineKeyboardButton::copy_text_button(
                format!("Copy this chat's ID ({chat_id})"),
                CopyTextButton {
                    text: chat_id.to_string(),
                },
            );
            let inline_keyboard = InlineKeyboardMarkup::new(vec![vec![copy_button]]);

            bot.send_message(
                chat_id,
                format!(
                    r#"<b>TrufotBot help</b>

{}"#,
                    Command::descriptions()
                ),
            )
            .reply_markup(inline_keyboard)
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
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
                    reminder_sent_time: None,
                }),
                State(storage),
                State(TelegramSender::new(bot.clone()).into()),
                State(config),
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

async fn inline_query_handler(storage: Storage, bot: Bot, q: InlineQuery) -> Result<()> {
    check_user(q.from.username.as_deref())?;

    let results = autocomplete::autocomplete(storage, &q.query)
        .await?
        .iter()
        .map(|completion| {
            InlineQueryResult::Article(InlineQueryResultArticle::new(
                completion,
                completion,
                InputMessageContent::Text(InputMessageContentText::new(completion)),
            ))
        })
        .collect::<Vec<_>>();

    let response = bot
        .answer_inline_query(q.id.clone(), results)
        .cache_time(0 /* TODO we want this longer outside of dev, right? */)
        .await;

    if let Err(err) = response {
        log::error!("Error in inline query handler: {err:?}")
    }
    Ok(())
}

const ALLOWED_USERS_ENV_VAR: &str = "TRUFOTBOT_ALLOWED_USERS";

fn check_user(user_id: Option<&str>) -> Result<()> {
    let Some(user_id) = user_id else {
        anyhow::bail!(
            "Couldn't check if user is allowed to send messages, \
            as user was None"
        );
    };
    let Ok(allowed_users) = std::env::var(ALLOWED_USERS_ENV_VAR) else {
        anyhow::bail!(
            "Couldn't check if {user_id:?} is allowed to send messages, \
            {ALLOWED_USERS_ENV_VAR} is not set"
        );
    };

    if allowed_users
        .split(",")
        .any(|allowed_user| allowed_user == user_id)
    {
        return Ok(());
    }

    anyhow::bail!("Forbidden user {user_id:?}; allowed users are {allowed_users}");
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
    let is_reply = msg.reply_to_message().is_some();
    let looks_like_command = matches!(msg.text(), Some(txt) if txt.starts_with("/"));
    if is_reply && !looks_like_command {
        return Ok(());
    }

    bot.send_message(msg.chat.id, "Please see /help".to_string())
        .reply_to(msg.id)
        .await?;
    Ok(())
}

async fn callback_handler(
    bot: Bot,
    q: CallbackQuery,
    storage: Storage,
    config: Arc<Config>,
) -> Result<()> {
    check_user(q.from.username.as_deref())?;

    let Some(ref data) = q.data else {
        return Ok(());
    };

    let message_id = q.regular_message().map(|message| message.id.0);
    let reminder_sent_time = q.regular_message().map(|message| message.date);

    if reminder_sent_time.is_none() || message_id.is_none() {
        log::warn!(
            "Received callback with message_id {message_id:?} and date {reminder_sent_time:?}; neither should be None"
        );
    }

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
                    reminder_sent_time,
                }),
                State(storage),
                State(TelegramSender::new(bot).into()),
                State(config),
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
