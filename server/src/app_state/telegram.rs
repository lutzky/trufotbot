use super::AppState;
use super::telegram_impl::MessageId;
use crate::models::Patient;
use axum::http::StatusCode;

pub trait SentMessageInfo {
    fn id(&self) -> MessageId;
}

impl SentMessageInfo for teloxide::types::Message {
    fn id(&self) -> MessageId {
        self.id.0
    }
}

impl SentMessageInfo for MessageId {
    fn id(&self) -> MessageId {
        *self
    }
}

impl AppState {
    pub async fn send_message(
        &self,
        patient: &Patient,
        message: String,
    ) -> Result<Option<impl SentMessageInfo>, (StatusCode, String)> {
        #[cfg(test)]
        {
            self.send_message_mock(patient, message).await
        }
        #[cfg(not(test))]
        {
            self.send_message_telegram(patient, message).await
        }
    }

    pub async fn edit_message(
        &self,
        patient: &Patient,
        message_id: MessageId,
        new_message: String,
    ) -> Result<(), (StatusCode, String)> {
        #[cfg(test)]
        {
            self.edit_message_mock(patient, message_id, new_message)
                .await
        }
        #[cfg(not(test))]
        {
            self.edit_message_telegram(patient, message_id, new_message)
                .await
        }
    }
}
