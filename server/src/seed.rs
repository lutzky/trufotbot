use chrono::{Duration, Utc};
use rand::Rng as _;
use sqlx::SqlitePool;

pub async fn seed_database(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let telegram_group_id = std::env::var("TELEGRAM_GROUP_ID")
        .expect("TELEGRAM_GROUP_ID must be set for seeding")
        .parse::<i64>()
        .expect("TELEGRAM_GROUP_ID must be a valid integer");

    log::info!("Inserting patients");
    sqlx::query!(
        "INSERT INTO patients (name, telegram_group_id) VALUES ('Alice', ?), ('Bob', ?), ('Carol', ?)",
        telegram_group_id,
        telegram_group_id,
        telegram_group_id
    )
    .execute(pool)
    .await?;

    log::info!("Inserting medications");
    sqlx::query!(
        "INSERT INTO medications (name, description) VALUES
            ('Aspirin', 'Pain reliever and anti-inflammatory'),
            ('Ibuprofen', 'Nonsteroidal anti-inflammatory drug'),
            ('Paracetamol', 'Pain reliever and fever reducer'),
            ('Amoxicillin', 'Antibiotic used to treat infections'),
            ('Metformin', 'Medication for type 2 diabetes management')"
    )
    .execute(pool)
    .await?;

    log::info!("Generating random dose history");
    let mut rng = rand::rng();
    let now = Utc::now().naive_utc();

    for patient_id in 1..=3 {
        for medication_id in 1..=5 {
            let num_doses = rng.random_range(5..=15);

            for _ in 0..num_doses {
                // Random time in the last 30 days
                let days_ago = rng.random_range(0..30);
                let hours = rng.random_range(6..23); // More realistic hours (6am to 11pm)
                let minutes = rng.random_range(0..60);
                let taken_at = now - Duration::days(days_ago)
                    + Duration::hours(hours)
                    + Duration::minutes(minutes);

                sqlx::query!(
                    "INSERT INTO doses (patient_id, medication_id, quantity, taken_at)
                     VALUES (?, ?, ?, ?)",
                    patient_id,
                    medication_id,
                    1.0, // Default quantity
                    taken_at,
                )
                .execute(pool)
                .await?;
            }
        }
    }

    Ok(())
}
