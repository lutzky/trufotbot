#![cfg(test)]

use axum::http::StatusCode;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

use crate::models::Patient;

use super::{Messenger, SentMessageInfo, callbacks, telegram_impl::MessageId};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct MessageWithKeyboard {
    id: i32,
    text: String,
    keyboard: Vec<(String, callbacks::Action)>,
}

#[derive(Default)]
struct GroupMessages {
    messages: Vec<MessageWithKeyboard>,
    last_id: i32,
}

pub fn messages_from_slice(v: &[(&str, &[(&str, callbacks::Action)])]) -> Vec<MessageWithKeyboard> {
    v.iter()
        .enumerate()
        .map(|(i, &(text, kbd))| MessageWithKeyboard {
            id: (i + 1).try_into().unwrap(),
            text: text.to_owned(),
            keyboard: kbd
                .iter()
                .map(|&(k, ref v)| (k.to_owned(), v.clone()))
                .collect::<Vec<_>>(),
        })
        .collect()
}

type MessageMap = HashMap<i64, GroupMessages>;

#[derive(Clone, Default)]
pub struct MessageHistory(Arc<Mutex<MessageMap>>);

impl MessageHistory {
    pub async fn add_message(&self, chat_id: i64, text: String) -> i32 {
        let mut messages = self.0.lock().await;

        let group_messages = messages
            .entry(chat_id)
            .or_insert_with(GroupMessages::default);
        group_messages.last_id += 1;

        group_messages.messages.push(MessageWithKeyboard {
            id: group_messages.last_id,
            text,
            ..Default::default()
        });

        group_messages.last_id
    }

    pub async fn get_messages(&self, chat_id: i64) -> Option<Vec<MessageWithKeyboard>> {
        let messages = self.0.lock().await;
        messages.get(&chat_id).map(|m| m.messages.clone())
    }

    pub async fn replace_message(
        &self,
        chat_id: i64,
        message_id: i32,
        new_text: String,
        new_keyboard: Vec<(String, callbacks::Action)>,
    ) -> Result<(), &str> {
        let mut messages = self.0.lock().await;

        let Some(group_messages) = messages.get_mut(&chat_id) else {
            return Err("Chat not found");
        };

        let Some(message) = group_messages
            .messages
            .iter_mut()
            .find(|m| m.id == message_id)
        else {
            return Err("Message not found in chat");
        };

        message.text = new_text;
        message.keyboard = new_keyboard;

        Ok(())
    }
}

impl Messenger {
    pub(super) async fn send_impl_mock(
        &self,
        patient: &Patient,
        message: String,
    ) -> Result<Option<impl SentMessageInfo>, (StatusCode, String)> {
        let Some(telegram_group_id) = patient.telegram_group_id else {
            log::warn!(
                "Patient {} has no telegram group ID, skipping message.",
                patient.name
            );
            return Ok(None);
        };

        let id = self
            .telegram_messages
            .add_message(telegram_group_id, message.clone())
            .await;

        Ok(Some(id))
    }

    pub(super) async fn edit_impl_mock(
        &self,
        patient: &Patient,
        message_id: MessageId,
        new_message: String,
        new_keyboard: Vec<(String, callbacks::Action)>,
    ) -> Result<(), (StatusCode, String)> {
        let Some(telegram_group_id) = patient.telegram_group_id else {
            log::warn!(
                "Patient {} has no telegram group ID, skipping message.",
                patient.name
            );
            return Ok(());
        };

        self.telegram_messages
            .replace_message(telegram_group_id, message_id, new_message, new_keyboard)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to replace message".to_string(),
                )
            })?;

        Ok(())
    }
}
