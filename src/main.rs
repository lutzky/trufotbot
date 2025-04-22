use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch},
    Json, Router,
};

// cspell: words sqlx dotenv chrono teloxide

use chrono::{NaiveDateTime, Utc};
use dotenv::dotenv;
use sqlx::{FromRow, SqlitePool};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer; // For CORS

mod models;
use models::{CreateIntake, Patient, UserMedicineDetails};

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    // Add Telegram bot client here later
    // telegram_bot: teloxide::Bot,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok(); // Load .env file

    // Set up the database connection pool
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let pool = SqlitePool::connect(&database_url).await?;

    // Run migrations on startup (optional, but good for development)
    sqlx::migrate!().run(&pool).await?;

    // TODO: Initialize Telegram bot client here

    let app_state = AppState {
        db: pool,
        // telegram_bot: ..., // Add initialized client
    };

    // Build the Axum application
    let app = Router::new()
        .route("/patients", get(list_patients))
        .route("/patients/{patient_id}", patch(update_patient))
        // TODO: There's some kind of standard for how to name these - https://stackoverflow.blog/2020/03/02/best-practices-for-rest-api-design/
        // .route(
        //     "/patients/:patient_id/medications",
        //     get(list_patient_medications),
        // )
        // .route(
        //     "/patients/:patient_id/medications/:medication_id",
        //     get(get_user_medicine_details),
        // )
        // .route(
        //     "/patients/:patient_id/medications/:medication_id",
        //     post(record_dose),
        // )
        .layer(CorsLayer::permissive()) // Allow all origins for simplicity during development // FIXME?
        .with_state(app_state);

    // Run the server
    // TODO: Make listening bind flag-configurable
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("Listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}

// --- API Handlers ---

async fn list_patients(
    State(state): State<AppState>,
) -> Result<Json<Vec<Patient>>, (StatusCode, String)> {
    let patients = sqlx::query_as!(Patient, "SELECT id, name, telegram_group_id FROM patients")
        .fetch_all(&state.db)
        .await
        .map_err(|e| {
            eprintln!("Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch users".to_string(),
            )
        })?;

    Ok(Json(patients))
}

#[derive(serde::Deserialize)]
struct UpdatePatient {
    name: Option<String>,
    telegram_group_id: Option<i64>,
}

#[axum::debug_handler]
async fn update_patient(
    State(state):State<AppState>,
    Path(patient_id): Path<i64>,
    Json(payload): Json<UpdatePatient>,
) -> Result<StatusCode, (StatusCode, String)> {
    let v = sqlx::query!(
        r#"
        UPDATE patients
        SET name = COALESCE(?, name),
            telegram_group_id = COALESCE(?, telegram_group_id)
        WHERE id = ?
        "#,
        payload.name,
        payload.telegram_group_id,
        patient_id
    ).execute(&state.db).await.map_err(|e| {
        eprintln!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update patient".to_string(),
        )
    })?;
    if v.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            "Patient not found".to_string(),
        ));
    }
    Ok(StatusCode::OK)
}

async fn list_patient_medications(
    Path(user_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<Json<Vec<UserMedicineDetails>>, (StatusCode, String)> {
    let medicines = sqlx::query_as::<_, UserMedicineDetails>(
        r#"
        SELECT
            um.user_id,
            um.medicine_id,
            m.name AS medicine_name,
            (SELECT taken_at FROM intake_records WHERE user_id = um.user_id AND medicine_id = um.medicine_id ORDER BY taken_at DESC LIMIT 1) AS last_taken_at,
            um.daily_reminder_hour,
            um.telegram_group_id,
            um.default_quantity
        FROM user_medicines um
        JOIN medicines m ON um.medicine_id = m.id
        WHERE um.user_id = ?
        "#
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        eprintln!("Database error: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch user medicines".to_string())
    })?;

    Ok(Json(medicines))
}

async fn get_user_medicine_details(
    Path((user_id, medicine_id)): Path<(i64, i64)>,
    State(state): State<AppState>,
) -> Result<Json<UserMedicineDetails>, (StatusCode, String)> {
    let details = sqlx::query_as::<_, UserMedicineDetails>(
        r#"
        SELECT
            um.user_id,
            um.medicine_id,
            m.name AS medicine_name,
            (SELECT taken_at FROM intake_records WHERE user_id = um.user_id AND medicine_id = um.medicine_id ORDER BY taken_at DESC LIMIT 1) AS last_taken_at,
            um.daily_reminder_hour,
            um.telegram_group_id,
            um.default_quantity
        FROM user_medicines um
        JOIN medicines m ON um.medicine_id = m.id
        WHERE um.user_id = ? AND um.medicine_id = ?
        "#
    )
    .bind(user_id)
    .bind(medicine_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
            sqlx::Error::RowNotFound => (StatusCode::NOT_FOUND, "User medicine details not found".to_string()),
            _ => {
                eprintln!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch user medicine details".to_string())
            }
        })?;

    Ok(Json(details))
}

async fn record_dose(
    Path((user_id, medicine_id)): Path<(i64, i64)>,
    State(state): State<AppState>,
    Json(payload): Json<CreateIntake>,
) -> Result<StatusCode, (StatusCode, String)> {
    todo!();
    // Basic validation: Check if user and medicine exist and are linked
    // let user_medicine = sqlx::query!(
    //     "SELECT CASE WHEN EXISTS(SELECT 1 FROM user_medicines WHERE user_id = ? AND medicine_id = ?) THEN 1 ELSE 0 END AS as_exists",
    //     user_id,
    //     medicine_id
    // )
    // .fetch_optional(&state.db)
    // .await
    // .map_err(|e| {
    //     eprintln!("Database error: {}", e);
    //     (StatusCode::INTERNAL_SERVER_ERROR, "Database check failed".to_string())
    // })?;

    // // TODO I am now checking the wrong thing?
    // if user_medicine.is_none() {
    //     return Err((
    //         StatusCode::BAD_REQUEST,
    //         "User is not associated with this medicine".to_string(),
    //     ));
    // }

    // // Insert the intake record
    // let result = sqlx::query!(
    //     r#"
    //     INSERT INTO intake_records (user_id, medicine_id, quantity, taken_at, noted_by_user_id)
    //     VALUES (?, ?, ?, ?, ?)
    //     "#,
    //     user_id,
    //     medicine_id,
    //     payload.quantity,
    //     payload.taken_at,
    //     payload.noted_by_user_id,
    // )
    // .execute(&state.db)
    // .await
    // .map_err(|e| {
    //     eprintln!("Database error: {}", e);
    //     (
    //         StatusCode::INTERNAL_SERVER_ERROR,
    //         "Failed to record intake".to_string(),
    //     )
    // })?;

    // let intake_id = result.last_insert_rowid(); // Get the ID of the new intake record

    // TODO: Telegram Notification Logic
    // Based on the user journeys:
    // 1. Fetch user and medicine names.
    // 2. Fetch the associated Telegram group ID from user_medicines.
    // 3. Construct the notification message (e.g., "Ohad noted that Ben took Paracetamol").
    //    Consider the timestamp logic (within 5 minutes = no time mentioned).
    // 4. Send the message to the Telegram group using `teloxide`.
    // 5. If this intake corresponds to a reminder message that was edited,
    //    you'll need a way to link the intake record to the original reminder message ID
    //    and use `teloxide` to edit that message. This is more complex and likely
    //    part of the reminder task's logic, which would store the message ID in the DB.
    //    For a direct intake recording, you might just send a new message.

    // Example placeholder for sending a Telegram message:
    /*
    if let Some(telegram_group_id) = user_medicine.telegram_group_id { // You'd need to fetch this
        let message_text = format!("Intake recorded for user {} and medicine {}!", user_id, medicine_id); // Improve this message
        // state.telegram_bot.send_message(telegram_group_id, message_text).await.expect("Failed to send telegram message");
    }
    */

    Ok(StatusCode::CREATED)
}

// TODO: Add endpoint for editing intake records if needed.
// TODO: Add endpoints for managing users and medicines via API (optional for base version)
