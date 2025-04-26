use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

#[derive(Clone)]
pub struct MessageHistory(Arc<Mutex<HashMap<i64, Vec<(i32, String)>>>>);

impl MessageHistory {
    pub fn new() -> Self {
        MessageHistory(Arc::new(Mutex::new(HashMap::new())))
    }

    pub async fn add_message(&self, chat_id: i64, text: String) -> i32 {
        let mut messages = self.0.lock().await;

        let group_messages = messages.entry(chat_id).or_insert_with(Vec::new);
        let message_id = group_messages.len() as i32 + 1; // FIXME: Collisions can occur after removals

        group_messages.push((message_id, text));

        message_id
    }

    pub async fn get_messages(&self, chat_id: i64) -> Option<Vec<(i32, String)>> {
        let messages = self.0.lock().await;
        messages.get(&chat_id).cloned()
    }
}
