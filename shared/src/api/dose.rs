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

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub struct AvailableDose {
    pub time: DateTime<Utc>,

    /// None means "we don't know the amount"
    pub quantity: Option<f64>,
}
