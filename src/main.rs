mod errors;
mod handlers;
mod middleware;
mod models;

use axum::{routing::{get, patch}, Router};
use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::{env, time::Duration};
use tower_cookies::CookieManagerLayer;
use tower_http::cors::CorsLayer;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;
use axum::http::{HeaderValue, Method, header};
use tracing_subscriber;

// ── App State ─────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

// ── Main ──────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

let pool = PgPoolOptions::new()
    .max_connections(20)
    .min_connections(5)
    .acquire_timeout(Duration::from_secs(5))
    .max_lifetime(Duration::from_secs(1800))
    .idle_timeout(Duration::from_secs(600))
    .connect(&database_url)
    .await
    .expect("Failed to connect to database");

// use std::time::Duration;

    println!("✅ Vendor API connected to database");


   // Migrations are owned by reader API — schema already exists

    let state = AppState { pool };

    // CORS — allow frontend dev server and production origin
    let frontend_origin = env::var("FRONTEND_URL")
        .unwrap_or_else(|_| "http://localhost:5173".into());

    let cors = CorsLayer::new()
        .allow_origin(frontend_origin.parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::COOKIE])
        .expose_headers([header::SET_COOKIE])
        .allow_credentials(true);

    let app = Router::new()
        // ── Vendor stats overview ──────────────────────────────────
        .route("/vendor/stats", get(handlers::get_stats))

        // ── Vendor order management ────────────────────────────────
        .route("/vendor/orders",            get(handlers::get_orders))
        .route("/vendor/orders/:id",        get(handlers::get_order))
        .route("/vendor/orders/:id/status", patch(handlers::update_order_status))

        // ── Vendor inventory ───────────────────────────────────────
        .route("/vendor/inventory",          get(handlers::get_inventory))
        .route("/vendor/inventory/:book_id", patch(handlers::update_inventory))

        // ── Admin — approve vendor applications ────────────────────
        .route(
            "/admin/vendor-applications/:id/approve",
            patch(handlers::approve_vendor_application),
        )

        .with_state(state)
        .layer(CookieManagerLayer::new())
        .layer(cors)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001")
        .await
        .unwrap();

    println!("🏪 Vendor API running at http://localhost:3001");
    println!();
    println!("   ── Vendor (role = vendor | admin) ───");
    println!("   GET    /vendor/stats");
    println!("   GET    /vendor/orders  [?status=&limit=&cursor=]");
    println!("   GET    /vendor/orders/:id");
    println!("   PATCH  /vendor/orders/:id/status");
    println!("   GET    /vendor/inventory");
    println!("   PATCH  /vendor/inventory/:book_id");
    println!();
    println!("   ── Admin (role = admin) ──────────────");
    println!("   PATCH  /admin/vendor-applications/:id/approve");

    axum::serve(listener, app).await.unwrap();
}