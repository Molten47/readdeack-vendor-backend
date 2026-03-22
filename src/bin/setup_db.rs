use sqlx::postgres::PgPoolOptions;
use std::env;
use dotenvy::dotenv;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in .env");

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    println!("✅ Connected to database");

    // Add role column to users
    sqlx::query(
        "ALTER TABLE users ADD COLUMN IF NOT EXISTS role TEXT NOT NULL DEFAULT 'reader'
         CHECK (role IN ('reader', 'vendor', 'admin'))"
    )
    .execute(&pool)
    .await
    .expect("Failed to add role column");
    println!("✅ Added role column to users");

    // Add owner_id to bookstores
    sqlx::query(
        "ALTER TABLE bookstores ADD COLUMN IF NOT EXISTS owner_id UUID
         REFERENCES users(id) ON DELETE SET NULL"
    )
    .execute(&pool)
    .await
    .expect("Failed to add owner_id column");
    println!("✅ Added owner_id column to bookstores");

    // Indexes
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_bookstores_owner
         ON bookstores(owner_id) WHERE owner_id IS NOT NULL"
    )
    .execute(&pool)
    .await
    .expect("Failed to create bookstores index");

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_users_role
         ON users(role) WHERE role != 'reader'"
    )
    .execute(&pool)
    .await
    .expect("Failed to create users role index");

    println!("✅ Indexes created");
    println!("🎉 Vendor schema setup complete — run cargo build now");
}