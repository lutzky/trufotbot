use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug, ToSchema)]
pub struct CreateDose {
    pub quantity: f64,
    pub taken_at: DateTime<Utc>,

    #[schema(examples("Alice", "Bob"))]
    pub noted_by_user: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug, ToSchema)]
pub struct Dose {
    pub id: i64,
    pub data: CreateDose,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug, ToSchema)]
pub struct AvailableDose {
    pub time: DateTime<Utc>,

    /// None means "we don't know the amount"
    pub quantity: Option<f64>,
}
