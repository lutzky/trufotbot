use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub struct CreateDose {
    pub quantity: f64,
    pub taken_at: DateTime<Utc>,
    pub noted_by_user: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub struct Dose {
    pub id: i64,
    pub data: CreateDose,
}

// TODO: Move to shared::api::doses, and make frontend use it
#[derive(Default, Deserialize, Serialize)]
pub struct CreateDoseQueryParams {
    pub reminder_message_id: Option<i32>,
}
