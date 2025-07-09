use sqlx::SqlitePool;

#[derive(Clone)]
pub struct Storage {
    pub pool: SqlitePool,
}
