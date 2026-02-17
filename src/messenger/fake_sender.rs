#![cfg(test)]

use anyhow::{Result, bail};
use async_trait::async_trait;
use std::{collections::HashMap, pin::Pin, sync::Arc};
use teloxide::types::ChatId;
use tokio::sync::Mutex;

use super::{MessageId, Sender, SentMessageInfo, callbacks};

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

#[derive(Default)]
pub struct MessageHistory(Mutex<MessageMap>);

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
    ) -> Result<()> {
        let mut messages = self.0.lock().await;

        let Some(group_messages) = messages.get_mut(&chat_id) else {
            bail!("Chat not found");
        };

        let Some(message) = group_messages
            .messages
            .iter_mut()
            .find(|m| m.id == message_id)
        else {
            bail!("Message not found in chat");
        };

        message.text = new_text;
        message.keyboard = new_keyboard;

        Ok(())
    }

    pub async fn delete_message(&self, chat_id: i64, message_id: i32) -> Result<()> {
        let mut messages = self.0.lock().await;

        let Some(group_messages) = messages.get_mut(&chat_id) else {
            bail!("Chat not found");
        };

        if let Some(pos) = group_messages
            .messages
            .iter()
            .position(|m| m.id == message_id)
        {
            group_messages.messages.swap_remove(pos);
        } else {
            bail!("Message not found in chat");
        }

        Ok(())
    }
}

/// Fake [`Sender`] that collects its messages, for later inspection by tests
#[derive(Default)]
pub struct FakeSender {
    pub messages: MessageHistory,
}

impl FakeSender {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
}

#[async_trait]
impl Sender for FakeSender {
    async fn send(
        &self,
        chat_id: ChatId,
        message: String,
    ) -> Result<Option<Pin<Box<dyn SentMessageInfo + Send>>>> {
        let id = self.messages.add_message(chat_id.0, message.clone()).await;

        Ok(Some(Box::pin(id)))
    }

    async fn edit(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        new_message: String,
        new_keyboard: Vec<(String, callbacks::Action)>,
    ) -> Result<()> {
        self.messages
            .replace_message(chat_id.0, message_id, new_message, new_keyboard)
            .await?;

        Ok(())
    }

    async fn delete(&self, chat_id: ChatId, message_id: MessageId) -> Result<()> {
        self.messages.delete_message(chat_id.0, message_id).await?;

        Ok(())
    }
}

impl From<FakeSender> for super::Messenger {
    fn from(value: FakeSender) -> Self {
        Self::new_from_sender(Arc::new(value))
    }
}

impl From<Arc<FakeSender>> for super::Messenger {
    fn from(value: Arc<FakeSender>) -> Self {
        Self::new_from_sender(value)
    }
}
