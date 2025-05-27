use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct MedicationSummary {
    pub id: i64,
    pub name: String,
    pub last_taken_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct DoseLimit {
    pub hours: u16,
    pub amount: f64,
}
