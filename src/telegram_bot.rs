// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use chrono::{DateTime, Duration, Local, TimeZone, Utc};
use std::sync::Arc;

use crate::{
    api::{dose::CreateDose, requests::CreateDoseQueryParams},
    app_state::Config,
    time::now,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use color_eyre::eyre::{Result, bail, eyre};
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

/// Handle a parsed bot command by either sending help text or recording a dose.
///
/// For `Command::Help` this sends an HTML-formatted help message with a button to copy
/// the current chat ID. For `Command::Record(...)` this verifies the patient and
/// medication exist, determines the `noted_by_user` and `taken_at` (defaulting to `now()`
/// when not provided), records the dose, and reacts to the triggering message on success.
///
/// # Errors
///
/// Returns an error if user authentication fails, if Telegram API calls fail, or if
/// recording the dose returns an error.
///
/// # Examples
///
/// ```
/// # use std::sync::Arc;
/// # use teloxide::prelude::*;
/// # async fn example(storage: crate::storage::Storage, config: Arc<crate::Config>, bot: Bot, msg: teloxide::types::Message, cmd: crate::Command) {
/// // In real usage this is awaited inside an async runtime:
/// let _ = crate::command_handler(storage, config, bot, msg, cmd).await;
/// # }
/// ```
async fn command_handler(
    storage: Storage,
    config: Arc<Config>,
    bot: Bot,
    msg: Message,
    cmd: Command,
) -> Result<()> {
    config.check_user(
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
        Command::Record(patient_name, medication_name, quantity, noted_by_user, taken_at) => {
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

            let noted_by_user = noted_by_user.or_else(|| {
                msg.from.map(|u| u.first_name).or_else(|| {
                    log::error!("Unexpected: msg.from is None in Command::Record handler");
                    None
                })
            });

            let taken_at = taken_at.unwrap_or_else(now);

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
                    taken_at,
                    noted_by_user,
                }),
            )
            .await
            .map_err(|e| {
                log::error!("Error recording dose from button: {e:?}");
                eyre!("Failed to record dose")
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

async fn inline_query_handler(
    config: Arc<Config>,
    storage: Storage,
    bot: Bot,
    q: InlineQuery,
) -> Result<()> {
    config.check_user(q.from.username.as_deref())?;

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

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    #[command(description = "display this text")]
    Help,
    #[command(
        description = r#"record a dose (e.g. /record Alice "Kids Paracetamol" 2 by Bob @10:00)"#,
        parse_with = parse_record_command
    )]
    Record(String, String, f64, Option<String>, Option<DateTime<Utc>>),
}

/// Parses the arguments of the `/record` command into its constituent fields.
///
/// The input may include an optional time suffix of the form `" @HH:MM"` and an optional
/// `" by <user>"` suffix. Returns a tuple `(patient_name, medication_name, quantity, noted_by_user, taken_at)`.
///
/// # Errors
///
/// Returns `ParseError::Custom` if tokenization fails, if the token count is not exactly three
/// (patient, medication, quantity), if `quantity` cannot be parsed as a number, or if a provided
/// time string cannot be parsed by `parse_time`.
///
/// # Examples
///
/// ```
/// let input = "Alice \"Vitamin D\" 2 by Bob @10:00".to_string();
/// let res = parse_record_command(input).unwrap();
/// assert_eq!(res.0, "Alice");
/// assert_eq!(res.1, "Vitamin D");
/// assert_eq!(res.2, 2.0);
/// assert_eq!(res.3.as_deref(), Some("Bob"));
/// assert!(res.4.is_some());
/// ```
#[allow(clippy::type_complexity)]
fn parse_record_command(
    s: String,
) -> Result<(String, String, f64, Option<String>, Option<DateTime<Utc>>), ParseError> {
    let (s, taken_at): (&str, Option<String>) = match s.split_once(" @") {
        Some((cmd, time)) => (cmd, Some(time.to_owned())),
        None => (s.as_ref(), None),
    };

    let (s, noted_by_user): (&str, Option<String>) = match s.split_once(" by ") {
        Some((cmd, user)) => (cmd, Some(user.to_owned())),
        None => (s, None),
    };

    let Some(sp) = shlex::split(s) else {
        return Err(ParseError::Custom(eyre!("shlex failed for {s:?}").into()));
    };
    let [patient_name, medication_name, quantity] = sp.as_slice() else {
        return Err(ParseError::Custom(
            eyre!("Got {} parameters (want 3)", sp.len()).into(),
        ));
    };

    let taken_at = match taken_at {
        Some(time_str) => Some(parse_time(&time_str)?),
        None => None,
    };

    Ok((
        patient_name.to_string(),
        medication_name.to_string(),
        quantity
            .parse()
            .map_err(|e| ParseError::Custom(eyre!("invalid quantity: {e}").into()))?,
        noted_by_user,
        taken_at,
    ))
}

/// Parse a local time in `HH:MM` format and convert it to a `DateTime<Utc>`, applying a simple day-rollover heuristic.
///
/// The function interprets the provided `time_str` as a local time on either today or yesterday:
/// - If the parsed local time is earlier than or equal to the current local time, it is treated as today.
/// - If the parsed local time is later than the current local time but within two hours, it is treated as today.
/// - If the parsed local time is more than two hours in the future, it is treated as yesterday.
///
/// Returns the corresponding `DateTime<Utc>` on success.
///
/// # Errors
///
/// Returns `Err(ParseError::Custom(...))` when:
/// - `time_str` is not in `HH:MM` form,
/// - hour or minute components cannot be parsed or are out of range (hour > 23 or minute > 59),
/// - the resulting local datetime is ambiguous or cannot be constructed.
///
/// # Examples
///
/// ```
/// // Basic successful parse; result should be a valid UTC datetime.
/// let dt = super::parse_time("10:00").expect("should parse 10:00");
/// assert!(dt.timestamp() > 0);
/// ```
fn parse_time(time_str: &str) -> Result<DateTime<Utc>, ParseError> {
    let local_now = Local::now();

    let hour_min: Vec<&str> = time_str.split(':').collect();

    let Some(&[hour_str, minute_str]) = hour_min.get(2..).map(|_| hour_min.as_slice()) else {
        return Err(ParseError::Custom(
            eyre!("invalid time format: {time_str}").into(),
        ));
    };

    let hour: u32 = hour_str
        .parse()
        .map_err(|e| ParseError::Custom(eyre!("invalid hour: {e}").into()))?;
    let minute: u32 = minute_str
        .parse()
        .map_err(|e| ParseError::Custom(eyre!("invalid minute: {e}").into()))?;

    if hour > 23 || minute > 59 {
        return Err(ParseError::Custom(
            eyre!("invalid time: {hour}:{minute:02}").into(),
        ));
    }

    let naive_time = local_now
        .date_naive()
        .and_hms_opt(hour, minute, 0)
        .ok_or_else(|| ParseError::Custom(eyre!("invalid time").into()))?;
    let local_now_naive = local_now.naive_local();

    let two_hours = Duration::hours(2);
    if naive_time > local_now_naive {
        let diff = naive_time.signed_duration_since(local_now_naive);
        if diff <= two_hours {
            let tz = Local.from_local_datetime(&naive_time).single();
            Ok(tz
                .ok_or_else(|| ParseError::Custom(eyre!("ambiguous local time").into()))?
                .with_timezone(&Utc))
        } else {
            let yesterday = local_now.date_naive() - chrono::TimeDelta::days(1);
            let yesterday_time = yesterday
                .and_hms_opt(hour, minute, 0)
                .ok_or_else(|| ParseError::Custom(eyre!("invalid time").into()))?;
            let tz = Local.from_local_datetime(&yesterday_time).single();
            Ok(tz
                .ok_or_else(|| ParseError::Custom(eyre!("ambiguous local time").into()))?
                .with_timezone(&Utc))
        }
    } else {
        let tz = Local.from_local_datetime(&naive_time).single();
        Ok(tz
            .ok_or_else(|| ParseError::Custom(eyre!("ambiguous local time").into()))?
            .with_timezone(&Utc))
    }
}

/// Replies with "Please see /help" to incoming messages unless the message is a reply that does not start with a slash.
///
/// # Examples
///
/// ```no_run
/// # use teloxide::Bot;
/// # use teloxide::types::Message;
/// # async fn example(bot: Bot, msg: Message) {
/// message_handler(bot, msg).await.unwrap();
/// # }
/// ```
///
/// @returns `Ok(())` on success, or an error if sending the reply fails.
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
    config.check_user(q.from.username.as_deref())?;

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
        action @ (callbacks::Action::TakeFromReminder {
            patient_id,
            medication_id,
            quantity,
        }
        | callbacks::Action::TakeNew {
            patient_id,
            medication_id,
            quantity,
        }) => {
            use callbacks::*;

            log::debug!("Received callback action: {action:?}");

            let create_dose_query_params = match action {
                callbacks::Action::TakeFromReminder { .. } => CreateDoseQueryParams {
                    reminder_message_id: message_id,
                    reminder_sent_time,
                },
                callbacks::Action::TakeNew { .. } => {
                    // This is not "from a reminder", but rather creating a completely new record;
                    // do not connect it to this message_id.
                    CreateDoseQueryParams::default()
                }
                callbacks::Action::Link { .. } => {
                    // Unreachable
                    bail!("Unexpected handler code called")
                }
            };

            let result = doses::record(
                Path((patient_id, medication_id)),
                Query(create_dose_query_params),
                State(storage),
                State(TelegramSender::new(bot.clone()).into()),
                State(config),
                Json(CreateDose {
                    quantity,
                    taken_at: now(),
                    noted_by_user: Some(q.from.first_name),
                }),
            )
            .await;

            // Stop the "spinner"; do this after we've attempted k
            bot.answer_callback_query(q.id)
                .text(match (&result, action) {
                    (Err(_), _) => "Something went wrong",
                    (Ok(_), Action::TakeNew { .. }) => "Dose recorded",
                    (Ok(_), Action::TakeFromReminder { .. }) => {
                        "Dose recorded, reminder marked done"
                    }
                    (Ok(_), Action::Link { .. }) => "Unexpected TrufotBot callback state",
                })
                .await?;

            result.map_err(|e| {
                log::error!("Error recording dose from button: {e:?}");
                eyre!("Failed to record dose")
            })?;
        }
        link_callback @ callbacks::Action::Link { .. } => {
            bail!("Unexpected callback {link_callback:?}")
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest(
        case("Alice Paracetamol 2"),
        case("Alice Paracetamol 2.5"),
        case("Alice Paracetamol 2 by Bob"),
    )]
    fn test_parse_record_command_without_time(#[case] input: &str) {
        let got = parse_record_command(input.to_string()).unwrap();
        assert!(got.4.is_none(), "expected no time for input: {input}");
    }

    #[rstest(
        case("Alice Paracetamol 2 @10:00"),
        case("Alice Paracetamol 2 by Bob @10:00"),
    )]
    fn test_parse_record_command_with_time(#[case] input: &str) {
        let got = parse_record_command(input.to_string()).unwrap();
        assert!(got.4.is_some(), "expected time for input: {input}");
    }

    #[test]
    fn test_parse_time_invalid_format() {
        let result = parse_time("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_time_invalid_hour() {
        let result = parse_time("25:00");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_time_invalid_minute() {
        let result = parse_time("10:60");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_time_valid() {
        let result = parse_time("10:00");
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert!(dt.timestamp() > 0);
    }
}
