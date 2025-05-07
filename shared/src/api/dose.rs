use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct CreateDose {
    pub quantity: f64,
    pub taken_at: DateTime<Utc>,
    pub noted_by_user: Option<String>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct Dose {
    pub id: i64,
    pub data: CreateDose,
}
