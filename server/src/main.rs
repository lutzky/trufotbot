use axum::Router;
use axum_embed::ServeEmbed;
use clap::Parser;
use rust_embed::RustEmbed;

// cspell: words sqlx dotenv chrono teloxide

use dotenv::dotenv;
use sqlx::SqlitePool;
use teloxide::Bot;
use tokio_cron_scheduler::{Job, JobScheduler};
use tower_http::cors::CorsLayer; // For CORS

mod app_state;
mod handlers;
mod models;
mod seed;
use app_state::AppState;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    seed: bool,
}

#[derive(RustEmbed, Clone)]
#[folder = "assets/"]
#[exclude = ".gitignore"]
struct Assets;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use axum::routing::{delete, get, post, put};

    let args = Args::parse();
    dotenv().ok(); // Load .env file

    pretty_env_logger::init();

    log::info!("Starting the server...");

    // Set up the database connection pool
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let pool = SqlitePool::connect(&database_url).await?;

    // Run migrations on startup (optional, but good for development)
    sqlx::migrate!().run(&pool).await?;

    if args.seed {
        log::info!("Seeding database...");
        seed::seed_database(&pool).await?;
        log::info!("Database seeded successfully!");
        return Ok(());
    }

    let telegram_bot = if std::env::var("TELOXIDE_TOKEN").is_ok() {
        Some(Bot::from_env())
    } else {
        log::warn!("TELOXIDE_TOKEN not set, Telegram bot functionality will be disabled.");
        None
    };

    let app_state = AppState::new(pool, telegram_bot);

    let serve_assets = ServeEmbed::<Assets>::with_parameters(
        // Return index.html for any path; that'll hit yew's BrowserRouter and
        // let it handle the routing.
        Some("index.html".to_owned()),
        axum_embed::FallbackBehavior::Ok,
        None,
    );

    // Build the Axum application
    let app = Router::new()
        .fallback_service(serve_assets)
        .route("/api/medications", post(handlers::medication::create))
        .route(
            "/api/medications/{medication_id}",
            delete(handlers::medication::delete),
        )
        .route("/api/patients", get(handlers::patient::list))
        .route("/api/patients", post(handlers::patient::create))
        .route("/api/patients/{patient_id}", get(handlers::patient::get))
        .route("/api/patients/{patient_id}", put(handlers::patient::update))
        .route(
            "/api/patients/{patient_id}",
            delete(handlers::patient::delete),
        )
        .route(
            "/api/patients/{patient_id}/ping",
            post(handlers::patient::ping),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}",
            put(handlers::medication::update),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}/doses",
            get(handlers::patient::doses::list),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}/doses",
            post(handlers::patient::doses::record),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}/doses/{dose_id}",
            get(handlers::patient::doses::get),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}/doses/{dose_id}",
            put(handlers::patient::doses::update),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}/doses/{dose_id}",
            delete(handlers::patient::doses::delete),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}/remind",
            put(handlers::patient::remind::send_reminder),
        )
        .layer(CorsLayer::permissive()) // Allow all origins for simplicity during development // FIXME?
        .with_state(app_state.clone());

    let sched = JobScheduler::new().await?;

    // TODO: Use something like this to allow english input in the UI with
    // client-side validation. Or, instead, use the english-to-cron crate
    // directly, which should be the underlying implementation. You don't want
    // to use cronjob syntax as the actual config, as it doesn't convert back to
    // english.
    // let k = tokio_cron_scheduler::job::JobLocked::schedule_to_cron("every hour");
    // dbg!(k);

    // TODO actually schedule based on patients' schedules
    sched
        .add(Job::new_async("every hour", {
            move |_uuid, _l| {
                Box::pin({
                    let app_state = app_state.clone();
                    async move {
                        if let Err(e) = handlers::patient::remind::send_reminder(
                            axum::extract::State(app_state),
                            axum::extract::Path((1, 1)),
                        )
                        .await
                        {
                            log::error!("Failed to send reminder: {e:?}");
                        }
                    }
                })
            }
        })?)
        .await?;

    sched.start().await?;

    // Run the server
    // TODO: Make listening bind flag-configurable
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    log::info!("Listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}

// TODO: Add endpoint for editing intake records if needed.
// TODO: Add endpoints for managing users and medicines via API (optional for base version)
