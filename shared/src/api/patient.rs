use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Patient {
    pub id: i64,
    pub name: String,
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Reminders {
    pub cron_schedules: Vec<String>,
}
