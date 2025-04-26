#![cfg(test)]

use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

#[derive(Default)]
struct GroupMessages {
    messages: Vec<(i32, String)>,
    last_id: i32,
}

type MessageMap = HashMap<i64, GroupMessages>;

#[derive(Clone)]
pub struct MessageHistory(Arc<Mutex<MessageMap>>);

impl MessageHistory {
    pub fn new() -> Self {
        MessageHistory(Arc::new(Mutex::new(MessageMap::new())))
    }

    pub async fn add_message(&self, chat_id: i64, text: String) -> i32 {
        let mut messages = self.0.lock().await;

        let group_messages = messages
            .entry(chat_id)
            .or_insert_with(GroupMessages::default);
        group_messages.last_id += 1;

        group_messages.messages.push((group_messages.last_id, text));

        group_messages.last_id
    }

    pub async fn get_messages(&self, chat_id: i64) -> Option<Vec<(i32, String)>> {
        let messages = self.0.lock().await;
        messages.get(&chat_id).map(|m| m.messages.clone())
    }

    pub async fn replace_message(
        &self,
        chat_id: i64,
        message_id: i32,
        new_message: String,
    ) -> Result<(), &str> {
        let mut messages = self.0.lock().await;

        let Some(group_messages) = messages.get_mut(&chat_id) else {
            return Err("Chat not found");
        };

        let Some(message) = group_messages
            .messages
            .iter_mut()
            .find(|m| m.0 == message_id)
        else {
            return Err("Message not found in chat");
        };

        message.1 = new_message;

        Ok(())
    }
}
