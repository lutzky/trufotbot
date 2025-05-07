use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct MedicationSummary {
    pub id: i64,
    pub name: String,
    pub last_taken_at: Option<DateTime<Utc>>,
}
