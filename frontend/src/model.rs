// TODO: Merge with the backend
#[derive(PartialEq, Clone, serde::Deserialize, Debug)] // Added Debug
pub struct Patient {
    pub id: i64,
    pub name: String,
    // Add other fields fetched from /api/patients/{id} later if needed
    // pub medications: Option<Vec<Medication>>,
}

// Placeholder for medication data later
// #[derive(PartialEq, Clone, serde::Deserialize, Debug)]
// pub struct Medication {
//   pub id: i64,
//   pub name: String,
//   // ... other details
// }

// Placeholder for dose logging info later
// #[derive(PartialEq, Clone, serde::Deserialize, Debug)]
// pub struct DoseInfo {
//   pub last_taken: Option<String>, // Use a proper DateTime type later
//   pub allowed_today: i32,
//   pub next_dose_wait_minutes: Option<i64>,
// }
