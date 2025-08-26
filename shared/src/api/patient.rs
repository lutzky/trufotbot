use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, ToSchema)]
pub struct Patient {
    pub id: i64,
    pub name: String,
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, ToSchema)]
pub struct Reminders {
    #[schema(examples("4:1,24:3"))]
    pub cron_schedules: Vec<String>,
}
