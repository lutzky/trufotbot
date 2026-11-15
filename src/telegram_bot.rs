// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use chrono::{DateTime, TimeZone, Utc};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::{
    api::{dose::CreateDose, requests::CreateDoseQueryParams},
    app_state::Config,
    ids::{MedicationId, PatientId, TelegramGroupId},
    natural_input::{self, Matchable, PlanResult},
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
    models::{DEFAULT_NATURAL_FROM_HISTORY_DAYS, Medication, NaturalFromHistoryResult, Patient},
    storage::Storage,
};

pub async fn launch(bot: Bot, storage: Storage, config: Arc<Config>) {
    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(command_handler),
        )
        .branch(Update::filter_message().endpoint(natural_message_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler))
        .branch(Update::filter_inline_query().endpoint(inline_query_handler));

    Dispatcher::builder(bot, handler)
        .dependencies(deps![storage.clone(), config.clone()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

const MAX_MEDICATIONS_LIST_CHARS: usize = 3000;

fn format_medications_with_labels(medications: &[Medication]) -> String {
    let joined = medications
        .iter()
        .map(|med| med.labels().collect::<Vec<_>>().join("/"))
        .collect::<Vec<_>>()
        .join(", ");

    let ellipsis = "...";
    if joined.chars().count() > MAX_MEDICATIONS_LIST_CHARS {
        let truncated: String = joined
            .chars()
            .take(MAX_MEDICATIONS_LIST_CHARS - ellipsis.len())
            .collect();
        format!("{truncated}{ellipsis}")
    } else {
        joined
    }
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
            let patients =
                Patient::find_by_telegram_group_id(&storage.pool, chat_id.into()).await?;
            let natural_medications = match patients.as_slice() {
                [patient] => Some(
                    patient
                        .get_natural_language_medications(&storage.pool, true)
                        .await?,
                ),
                _ => None,
            };

            let patients = match patients.is_empty() {
                false => format!(
                    "Patients in this chat: {}",
                    patients
                        .into_iter()
                        .map(|p| p.name)
                        .collect::<Vec<String>>()
                        .join(", ")
                ),
                true => "No patients in this chat.".to_string(),
            };

            let medications = match natural_medications {
                None => {
                    "Natural medication input disabled (requires exactly 1 patient)".to_string()
                }
                Some(medications) if medications.is_empty() => {
                    "No medications with natural input enabled (use /natural_from_history to enable recently used medications).".to_string()
                }
                Some(medications) => {
                    format!(
                        "Medications with natural input enabled: {}",
                        format_medications_with_labels(&medications)
                    )
                }
            };

            bot.send_message(
                chat_id,
                format!(
                    r#"<b>TrufotBot help</b> ({})

{}
{}

{}"#,
                    env!("VERGEN_GIT_DESCRIBE"),
                    teloxide::utils::html::escape(&patients),
                    teloxide::utils::html::escape(&medications),
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

            record_from_message(
                &bot,
                &msg,
                storage,
                config,
                (patient.id, medication.id),
                quantity,
                (noted_by_user, taken_at),
            )
            .await?;
        }
        Command::NaturalFromHistory(NaturalFromHistoryArgs { patient_name, days }) => {
            let patient = match patient_name {
                Some(name) => {
                    let Some(patient) = Patient::find_by_name(&storage.pool, &name).await? else {
                        bot.send_message(msg.chat.id, format!("Error: No such patient {name:?}"))
                            .reply_to(msg.id)
                            .await?;
                        return Ok(());
                    };
                    patient
                }
                None => {
                    let group_patients =
                        Patient::find_by_telegram_group_id(&storage.pool, msg.chat.id.into())
                            .await?;
                    match <[Patient; 1]>::try_from(group_patients) {
                        Ok([patient]) => patient,
                        Err(_) => {
                            bot.send_message(
                                msg.chat.id,
                                "Please specify a patient with /natural_from_history <patient> [days].",
                            )
                            .reply_to(msg.id)
                            .await?;
                            return Ok(());
                        }
                    }
                }
            };

            let res = patient
                .enable_natural_input_from_history(&storage.pool, days)
                .await?;
            let reply = format_natural_from_history_reply(&patient.name, days, &res);
            bot.send_message(msg.chat.id, reply)
                .reply_to(msg.id)
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
#[command(rename_rule = "snake_case")]
enum Command {
    #[command(description = "display this text")]
    Help,
    #[command(
        description = r#"record a dose (e.g. /record Alice "Kids Paracetamol" 2 by Bob @10:00)"#,
        parse_with = parse_record_command
    )]
    Record(RecordArgs),
    #[command(
        description = "enable natural input from recent dose history (e.g. /natural_from_history [days])",
        parse_with = parse_natural_from_history_command
    )]
    NaturalFromHistory(NaturalFromHistoryArgs),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NaturalFromHistoryArgs {
    patient_name: Option<String>,
    days: u32,
}

fn parse_days(s: &str) -> Result<u32, ParseError> {
    let days: u32 = s
        .parse()
        .map_err(|e| ParseError::Custom(eyre!("invalid days {s:?}: {e}").into()))?;
    if days == 0 {
        return Err(ParseError::Custom(
            eyre!("days must be greater than 0").into(),
        ));
    }
    Ok(days)
}

fn parse_natural_from_history_command(s: String) -> Result<(NaturalFromHistoryArgs,), ParseError> {
    let Some(sp) = shlex::split(&s) else {
        return Err(ParseError::Custom(eyre!("shlex failed for {s:?}").into()));
    };

    let args = match sp.as_slice() {
        [] => NaturalFromHistoryArgs {
            patient_name: None,
            days: DEFAULT_NATURAL_FROM_HISTORY_DAYS,
        },
        [single] => {
            if single.starts_with(|c: char| c.is_ascii_digit() || c == '-') {
                NaturalFromHistoryArgs {
                    patient_name: None,
                    days: parse_days(single)?,
                }
            } else {
                NaturalFromHistoryArgs {
                    patient_name: Some(single.to_string()),
                    days: DEFAULT_NATURAL_FROM_HISTORY_DAYS,
                }
            }
        }
        [patient_name, days_str] => NaturalFromHistoryArgs {
            patient_name: Some(patient_name.to_string()),
            days: parse_days(days_str)?,
        },
        _ => {
            return Err(ParseError::Custom(
                eyre!(
                    "Got {} parameters (want 0 to 2: [patient] [days])",
                    sp.len()
                )
                .into(),
            ));
        }
    };

    Ok((args,))
}

fn format_natural_from_history_reply(
    patient_name: &str,
    days: u32,
    res: &NaturalFromHistoryResult,
) -> String {
    let mut lines = vec![format!(
        "Natural input scan for {patient_name} (last {days} days):"
    )];

    if res.enabled.is_empty() {
        lines.push("Enabled: none".to_string());
    } else {
        let names = res
            .enabled
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("Enabled: {names}"));
    }

    if !res.skipped.is_empty() {
        lines.push("Skipped:".to_string());
        for s in &res.skipped {
            lines.push(format!("- {}: {}", s.name, s.reason));
        }
    }

    lines.push(format!("Already enabled: {}", res.already_enabled));
    lines.join("\n")
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

impl Matchable for Medication {
    fn labels(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.name.as_str()).chain(self.aliases.iter().map(|s| s.as_str()))
    }
}

async fn react(bot: &Bot, msg: &Message, emoji: impl Into<String>) {
    let emoji = emoji.into();
    // We explicitly log the emoji on failure, as Telegram can be picky about which emoji reactions
    // it allows.
    let emoji_for_log = emoji.clone();

    let result = bot
        .set_message_reaction(msg.chat.id, msg.id)
        .reaction(vec![ReactionType::Emoji { emoji }])
        .await;

    if let Err(err) = result {
        log::error!("Failed to react {emoji_for_log:?}: {err:?}");
        log::debug!("Failed to react {emoji_for_log:?} on {msg:?}: {err:?}");
    }
}

async fn record_from_message(
    bot: &Bot,
    msg: &Message,
    storage: Storage,
    config: Arc<Config>,
    (patient_id, medication_id): (PatientId, MedicationId),
    quantity: f64,
    (noted_by_user, taken_at): (Option<String>, Option<DateTime<Utc>>),
) -> Result<()> {
    let noted_by_user = noted_by_user.or_else(|| {
        let user = msg.from.as_ref().map(|u| u.first_name.clone());
        if user.is_none() {
            log::debug!("Message {msg:?} has empty `from`");
        }
        user
    });
    let taken_at = taken_at.unwrap_or_else(crate::time::now);

    doses::record(
        Path((patient_id, medication_id)),
        Query(CreateDoseQueryParams::default()),
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
        log::error!("Error recording dose from message {:?}: {e:?}", msg.text());
        eyre!("Failed to record dose")
    })?;

    react(bot, msg, "👍").await;
    Ok(())
}

#[derive(Debug, PartialEq)]
enum NaturalMessageOutcome {
    NotADose,
    NoMatch,
    NotEnabled {
        reply: String,
    },
    Ambiguous {
        reply: String,
    },
    Match {
        medication_id: MedicationId,
        quantity: f64,
    },
}

async fn resolve_natural_message(
    db: &SqlitePool,
    patient: &Patient,
    message_text: &str,
) -> Result<NaturalMessageOutcome> {
    let medications = patient.get_natural_language_medications(db, true).await?;
    log::debug!("Patient is {patient:?}. Natural language medications: {medications:?}");

    match natural_input::plan::<Medication>(message_text, &medications) {
        PlanResult::NotADose => Ok(NaturalMessageOutcome::NotADose),
        PlanResult::Match {
            candidate,
            quantity,
            ..
        } => Ok(NaturalMessageOutcome::Match {
            medication_id: candidate.id,
            quantity,
        }),
        PlanResult::Ambiguous {
            candidates: tied, ..
        } => {
            let names: Vec<String> = tied
                .iter()
                .map(|m| {
                    if m.matched_label.eq_ignore_ascii_case(&m.candidate.name) {
                        m.candidate.name.clone()
                    } else {
                        format!("{} ({})", m.matched_label, m.candidate.name)
                    }
                })
                .collect();
            log::info!("Ambiguous match for {message_text:?}: {names:?}");
            Ok(NaturalMessageOutcome::Ambiguous {
                reply: format!(
                    "I'm not sure which medication you meant — could be {}. Can you rephrase?",
                    names.join(" or ")
                ),
            })
        }
        PlanResult::NoMatch => {
            let disabled = patient.get_natural_language_medications(db, false).await?;
            let matched_disabled = match natural_input::plan::<Medication>(message_text, &disabled)
            {
                PlanResult::Match { candidate, .. } => Some(candidate.name.clone()),
                PlanResult::Ambiguous {
                    candidates: tied, ..
                } => Some(
                    tied.iter()
                        .map(|m| m.candidate.name.as_str())
                        .collect::<Vec<_>>()
                        .join(" or "),
                ),
                PlanResult::NoMatch | PlanResult::NotADose => None,
            };
            if let Some(matched_disabled) = matched_disabled {
                Ok(NaturalMessageOutcome::NotEnabled {
                    reply: format!(
                        "That doesn't match anything enabled for natural input for {}. If enabled, it could've matched {matched_disabled}.",
                        patient.name
                    ),
                })
            } else {
                Ok(NaturalMessageOutcome::NoMatch)
            }
        }
    }
}

async fn natural_message_handler(
    bot: Bot,
    msg: Message,
    storage: Storage,
    config: Arc<Config>,
) -> Result<()> {
    let Some(message_text) = msg.text() else {
        log::debug!("Ignoring non-text message {msg:?}");
        return Ok(());
    };

    // Replies are ignored (unless they start with `/`, which falls through to shrug below).
    if msg.reply_to_message().is_some() && !message_text.starts_with('/') {
        return Ok(());
    }

    // Chatter from non-allowed members shouldn't spam logs.
    if let Err(err) = config.check_user(
        msg.from
            .as_ref()
            .map(|u| &u.username)
            .unwrap_or(&None)
            .as_deref(),
    ) {
        log::debug!("User not allowed in natural_message_handler: {err:?}");
        return Ok(());
    }

    // Private chats: natural input is ignored, and we reply with /help.
    if msg.chat.is_private() {
        bot.send_message(msg.chat.id, "Please see /help".to_string())
            .reply_to(msg.id)
            .await?;
        return Ok(());
    }

    // Unrecognized slash commands fall through to this handler and get a shrug.
    if message_text.starts_with('/') {
        log::debug!(
            "Ignoring unrecognized command which fell through to natural_message_handler: {message_text:?}"
        );
        react(&bot, &msg, "🤷").await;
        return Ok(());
    }

    let db = &storage.pool;
    let telegram_group_id: TelegramGroupId = msg.chat.id.into();

    let patients = Patient::find_by_telegram_group_id(db, telegram_group_id).await?;
    let [patient] = patients.as_slice() else {
        log::debug!(
            "{telegram_group_id:?} doesn't have exactly 1 patient, skipping natural processing. Patients are: {patients:?}"
        );
        return Ok(());
    };

    match resolve_natural_message(db, patient, message_text).await? {
        NaturalMessageOutcome::NotADose => Ok(()),
        NaturalMessageOutcome::NoMatch => {
            react(&bot, &msg, "🤷").await;
            Ok(())
        }
        NaturalMessageOutcome::NotEnabled { reply }
        | NaturalMessageOutcome::Ambiguous { reply } => {
            bot.send_message(msg.chat.id, reply)
                .reply_to(msg.id)
                .await?;
            Ok(())
        }
        NaturalMessageOutcome::Match {
            medication_id,
            quantity,
        } => {
            record_from_message(
                &bot,
                &msg,
                storage,
                config,
                (patient.id, medication_id),
                quantity,
                (None, None),
            )
            .await
        }
    }
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

    let message_id = q
        .regular_message()
        .map(|message| message.id.0)
        .map(crate::ids::MessageId);
    let reminder_sent_time = q.regular_message().map(|message| message.date);

    if reminder_sent_time.is_none() || message_id.is_none() {
        log::warn!(
            concat!(
                "Received callback with message_id {message_id:?} and date ",
                "{reminder_sent_time:?}; neither should be None",
            ),
            message_id = message_id,
            reminder_sent_time = reminder_sent_time,
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

    #[rstest]
    #[case("", None, 90)]
    #[case("   ", None, 90)]
    #[case("30", None, 30)]
    #[case("Alice", Some("Alice"), 90)]
    #[case("Alice 45", Some("Alice"), 45)]
    #[case(r#""Alice Smith" 14"#, Some("Alice Smith"), 14)]
    fn test_parse_natural_from_history_valid(
        #[case] input: &str,
        #[case] want_patient: Option<&str>,
        #[case] want_days: u32,
    ) {
        let (got,) = parse_natural_from_history_command(input.to_string()).unwrap();
        assert_eq!(
            got,
            NaturalFromHistoryArgs {
                patient_name: want_patient.map(str::to_string),
                days: want_days,
            }
        );
    }

    #[rstest]
    #[case("0")]
    #[case("-5")]
    #[case("123abc")]
    #[case("Alice 0")]
    #[case("Alice -1")]
    #[case("Alice abc")]
    #[case("Alice 30 extra")]
    #[case("\"unclosed")]
    fn test_parse_natural_from_history_invalid(#[case] input: &str) {
        assert!(
            parse_natural_from_history_command(input.to_string()).is_err(),
            "expected error for {input:?}"
        );
    }

    #[test]
    fn test_command_parse_natural_from_history_and_existing_commands() {
        assert!(matches!(
            Command::parse("/help", "trufotbot").unwrap(),
            Command::Help
        ));
        assert!(matches!(
            Command::parse("/natural_from_history", "trufotbot").unwrap(),
            Command::NaturalFromHistory(NaturalFromHistoryArgs {
                patient_name: None,
                days: 90,
            })
        ));
        assert!(matches!(
            Command::parse("/natural_from_history 30", "trufotbot").unwrap(),
            Command::NaturalFromHistory(NaturalFromHistoryArgs {
                patient_name: None,
                days: 30,
            })
        ));
        assert!(matches!(
            Command::parse("/natural_from_history Alice 15", "trufotbot").unwrap(),
            Command::NaturalFromHistory(NaturalFromHistoryArgs {
                patient_name: Some(ref p),
                days: 15,
            }) if p == "Alice"
        ));
    }

    #[test]
    fn test_format_natural_from_history_reply() {
        use crate::ids::MedicationId;
        use crate::models::{EnabledMedication, SkippedMedication};

        let empty = NaturalFromHistoryResult {
            enabled: vec![],
            skipped: vec![],
            already_enabled: 2,
        };
        assert_eq!(
            format_natural_from_history_reply("Alice", 90, &empty),
            "Natural input scan for Alice (last 90 days):\nEnabled: none\nAlready enabled: 2"
        );

        let mixed = NaturalFromHistoryResult {
            enabled: vec![
                EnabledMedication {
                    id: MedicationId(1),
                    name: "Aspirin".to_string(),
                },
                EnabledMedication {
                    id: MedicationId(2),
                    name: "Ibuprofen".to_string(),
                },
            ],
            skipped: vec![SkippedMedication {
                id: MedicationId(3),
                name: "Paracetamol".to_string(),
                reason: r#"Label "Painkiller" collides with preexisting label "Painkiller" of "Ibuprofen""#.to_string(),
            }],
            already_enabled: 1,
        };
        assert_eq!(
            format_natural_from_history_reply("Alice", 30, &mixed),
            concat!(
                "Natural input scan for Alice (last 30 days):\n",
                "Enabled: Aspirin, Ibuprofen\n",
                "Skipped:\n",
                "- Paracetamol: Label \"Painkiller\" collides with preexisting label \"Painkiller\" of \"Ibuprofen\"\n",
                "Already enabled: 1",
            )
        );
    }

    // cSpell: word Acamol Nurofen Unobtainium
    #[sqlx::test(fixtures("fixtures/patients.sql", "fixtures/medications.sql"))]
    async fn resolve_natural_message_matches_enabled_and_reports_not_enabled_for_disabled(
        db: SqlitePool,
    ) {
        // Alice (1): Aspirin (1) enabled, Ibuprofen (2) no row (disabled),
        // Paracetamol (3) explicitly natural_input = false with alias "Acamol".
        sqlx::query!(
            r#"
            INSERT INTO patients_medications (patient_id, medication_id, reminder_cron_schedule, natural_input)
            VALUES (1, 1, '', true), (1, 3, '', false)
            "#
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query!("UPDATE medications SET aliases = 'Acamol' WHERE id = 3")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query!(
            "INSERT INTO medications (name, dose_limits) VALUES ('Nurofen Kids', ''), ('Nurofen Junior', '')"
        )
        .execute(&db)
        .await
        .unwrap();

        let alice = Patient::get(&db, PatientId(1)).await.unwrap();

        assert_eq!(
            resolve_natural_message(&db, &alice, "Aspirin 2")
                .await
                .unwrap(),
            NaturalMessageOutcome::Match {
                medication_id: MedicationId(1),
                quantity: 2.0,
            }
        );

        assert_eq!(
            resolve_natural_message(&db, &alice, "Ibuprofen 1")
                .await
                .unwrap(),
            NaturalMessageOutcome::NotEnabled {
                reply: "That doesn't match anything enabled for natural input for Alice. If enabled, it could've matched Ibuprofen.".to_string(),
            }
        );

        assert_eq!(
            resolve_natural_message(&db, &alice, "Acamol 5")
                .await
                .unwrap(),
            NaturalMessageOutcome::NotEnabled {
                reply: "That doesn't match anything enabled for natural input for Alice. If enabled, it could've matched Paracetamol.".to_string(),
            }
        );

        assert_eq!(
            resolve_natural_message(&db, &alice, "Nurofen 5")
                .await
                .unwrap(),
            NaturalMessageOutcome::NotEnabled {
                reply: "That doesn't match anything enabled for natural input for Alice. If enabled, it could've matched Nurofen Kids or Nurofen Junior.".to_string(),
            }
        );

        assert_eq!(
            resolve_natural_message(&db, &alice, "Unobtainium 5")
                .await
                .unwrap(),
            NaturalMessageOutcome::NoMatch
        );

        assert_eq!(
            resolve_natural_message(&db, &alice, "just chatting")
                .await
                .unwrap(),
            NaturalMessageOutcome::NotADose
        );
    }

    #[sqlx::test(fixtures("fixtures/patients.sql", "fixtures/medications.sql"))]
    async fn resolve_natural_message_reports_ambiguous_candidates(db: SqlitePool) {
        let kids_id =
            sqlx::query!("INSERT INTO medications (name, dose_limits) VALUES ('Nurofen Kids', '')")
                .execute(&db)
                .await
                .unwrap()
                .last_insert_rowid();
        let junior_id = sqlx::query!(
            "INSERT INTO medications (name, dose_limits) VALUES ('Nurofen Junior', '')"
        )
        .execute(&db)
        .await
        .unwrap()
        .last_insert_rowid();

        sqlx::query!(
            r#"
            INSERT INTO patients_medications (patient_id, medication_id, reminder_cron_schedule, natural_input)
            VALUES (1, ?, '', true), (1, ?, '', true)
            "#,
            kids_id,
            junior_id
        )
        .execute(&db)
        .await
        .unwrap();

        let alice = Patient::get(&db, PatientId(1)).await.unwrap();
        assert_eq!(
            resolve_natural_message(&db, &alice, "Nurofen 5")
                .await
                .unwrap(),
            NaturalMessageOutcome::Ambiguous {
                reply: "I'm not sure which medication you meant — could be Nurofen Kids or Nurofen Junior. Can you rephrase?".to_string(),
            }
        );
    }

    #[test]
    fn test_format_medications_with_labels_and_truncation() {
        use crate::api::medication::DoseLimits;

        let make_med = |id: i64, name: &str, aliases: &[&str]| Medication {
            id: MedicationId(id),
            name: name.to_string(),
            description: None,
            dose_limits: DoseLimits::default(),
            inventory: None,
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
        };

        let short = vec![
            make_med(1, "Aspirin", &["Asa"]),
            make_med(2, "Ibuprofen", &[]),
        ];
        assert_eq!(
            format_medications_with_labels(&short),
            "Aspirin/Asa, Ibuprofen"
        );

        let long_name = "M".repeat(60);
        let many: Vec<Medication> = (1..=80)
            .map(|i| make_med(i, &format!("{long_name}-{i}"), &[]))
            .collect();
        let formatted = format_medications_with_labels(&many);
        assert_eq!(formatted.chars().count(), MAX_MEDICATIONS_LIST_CHARS);
        assert!(formatted.ends_with("..."));
    }
}
