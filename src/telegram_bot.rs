// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use chrono::{DateTime, TimeZone, Utc};
use std::sync::Arc;

use crate::{
    api::{dose::CreateDose, requests::CreateDoseQueryParams},
    app_state::Config,
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
        Command::Record(RecordArgs {
            patient_name,
            medication_name,
            quantity,
            noted_by_user,
            taken_at,
        }) => {
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

            let taken_at = taken_at.unwrap_or_else(crate::time::now);

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
    Record(RecordArgs),
}

#[derive(Clone)]
struct RecordArgs {
    patient_name: String,
    medication_name: String,
    quantity: f64,
    noted_by_user: Option<String>,
    taken_at: Option<DateTime<Utc>>,
}

fn parse_record_command(s: String) -> Result<(RecordArgs,), ParseError> {
    let Some(sp) = shlex::split(&s) else {
        return Err(ParseError::Custom(eyre!("shlex failed for {s:?}").into()));
    };

    let mut tokens = sp.as_slice();

    let taken_at = match tokens {
        [rest @ .., last] => {
            if let Some(time) = last.strip_prefix('@') {
                tokens = rest;
                Some(time)
            } else {
                None
            }
        }
        _ => None,
    };

    let noted_by_user = match tokens {
        [rest @ .., by, user] if by == "by" => {
            tokens = rest;
            Some(user.to_owned())
        }
        _ => None,
    };

    let [patient_name, medication_name, quantity] = tokens else {
        return Err(ParseError::Custom(
            eyre!("Got {} parameters (want 3)", sp.len()).into(),
        ));
    };

    let taken_at = taken_at.map(parse_time).transpose()?;

    Ok((RecordArgs {
        patient_name: patient_name.to_string(),
        medication_name: medication_name.to_string(),
        quantity: quantity
            .parse()
            .map_err(|e| ParseError::Custom(eyre!("invalid quantity: {e}").into()))?,
        noted_by_user,
        taken_at,
    },))
}

fn parse_time(time_str: &str) -> Result<DateTime<Utc>, ParseError> {
    let now = crate::time::now()
        .with_timezone(&crate::time::local_timezone())
        .naive_local();

    let hour_min: Vec<&str> = time_str.split(':').collect();

    let [hour_str, minute_str] = hour_min.as_slice() else {
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

    let max_acceptable_future = chrono::TimeDelta::hours(2);

    let this_time_in_n_days = |days: i64| {
        (now.date() + chrono::TimeDelta::days(days))
            .and_hms_opt(hour, minute, 0)
            .ok_or_else(|| ParseError::Custom(eyre!("invalid time").into()))
    };

    let [yesterday_time, today_time, tomorrow_time] = [
        this_time_in_n_days(-1)?,
        this_time_in_n_days(0)?,
        this_time_in_n_days(1)?,
    ];

    let diff_today = today_time.signed_duration_since(now);
    let diff_tomorrow = tomorrow_time.signed_duration_since(now);

    let target_time = if diff_tomorrow <= max_acceptable_future {
        tomorrow_time
    } else if diff_today <= max_acceptable_future {
        today_time
    } else {
        yesterday_time
    };

    let tz = crate::time::local_timezone()
        .from_local_datetime(&target_time)
        .single();
    Ok(tz
        .ok_or_else(|| ParseError::Custom(eyre!("ambiguous local time").into()))?
        .with_timezone(&Utc))
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
                    taken_at: crate::time::now(),
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
    use crate::time::FAKE_TIME;

    use super::*;
    use rstest::rstest;

    #[rstest(
        case("Alice Paracetamol 2"),
        case("Alice Paracetamol 2.5"),
        case("Alice Paracetamol 2 by Bob")
    )]
    fn test_parse_record_command_without_time(#[case] input: &str) {
        let (got,) = FAKE_TIME.sync_scope("2025-01-01T10:00:00Z", || {
            parse_record_command(input.to_string()).unwrap()
        });
        assert!(
            got.taken_at.is_none(),
            "expected no time for input: {input}"
        );
    }

    #[rstest(
        case("Alice Paracetamol 2 @10:00"),
        case("Alice Paracetamol 2 by Bob @10:00")
    )]
    fn test_parse_record_command_with_time(#[case] input: &str) {
        let (got,) = FAKE_TIME.sync_scope("2025-01-01T10:00:00Z", || {
            parse_record_command(input.to_string()).unwrap()
        });
        assert!(got.taken_at.is_some(), "expected time for input: {input}");
    }

    #[test]
    fn test_parse_time_invalid_format() {
        let result = FAKE_TIME.sync_scope("2025-01-01T10:00:00Z", || parse_time("invalid"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_time_invalid_hour() {
        let result = FAKE_TIME.sync_scope("2025-01-01T10:00:00Z", || parse_time("25:00"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_time_invalid_minute() {
        let result = FAKE_TIME.sync_scope("2025-01-01T10:00:00Z", || parse_time("10:60"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_time_valid() {
        let result = FAKE_TIME.sync_scope("2025-01-01T10:00:00Z", || parse_time("10:00"));
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert!(dt.timestamp() > 0);
    }

    #[rstest]
    #[case::same_time_as_now("2025-01-02T10:00:00Z", "10:00", "2025-01-02T10:00:00Z")]
    #[case::one_hour_ago("2025-01-02T10:00:00Z", "09:00", "2025-01-02T09:00:00Z")]
    #[case::future_way_too_far_pick_today("2025-01-02T10:00:00Z", "04:00", "2025-01-02T04:00:00Z")]
    #[case::future_near_pick_today("2025-01-02T10:00:00Z", "11:00", "2025-01-02T11:00:00Z")]
    #[case::future_too_far_pick_yesterday("2025-01-02T10:00:00Z", "13:00", "2025-01-01T13:00:00Z")]
    #[case::midnight_crossing_future_near_pick_tomorrow(
        "2025-01-01T23:00:00Z",
        "00:30",
        "2025-01-02T00:30:00Z"
    )]
    #[case::midnight_crossing_future_too_far_pick_today(
        "2025-01-01T23:00:00Z",
        "01:30",
        "2025-01-01T01:30:00Z"
    )]
    fn test_parse_time_specific(
        #[case] fake_time: &'static str,
        #[case] input_time: &'static str,
        #[case] expected_utc: &'static str,
    ) {
        let result = FAKE_TIME.sync_scope(fake_time, || parse_time(input_time));
        assert!(result.is_ok(), "parse_time failed: {:?}", result);
        let dt = result.unwrap();
        let expected = chrono::DateTime::parse_from_rfc3339(expected_utc)
            .unwrap()
            .to_utc();
        assert_eq!(
            dt, expected,
            "For fake_time={fake_time}, input={input_time}"
        );
    }
}
