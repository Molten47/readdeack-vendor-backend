use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── JWT Claims — must match reader-api exactly ────────────────────
// Both services sign/verify with the same JWT_SECRET and same Claims shape.
// If reader-api Claims changes, update this too.

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub:      String,   // user UUID as string
    pub username: String,
    pub role:     String,   // "reader" | "vendor" | "admin" — added for vendor service
    pub exp:      usize,
}

// ── Vendor context — injected by VendorAuth extractor ────────────

#[derive(Debug, Clone)]
pub struct VendorUser {
    pub user_id:      Uuid,
    pub _username:     String,
    pub bookstore_id: Option<Uuid>// the bookstore this vendor owns
}

// ── Order models ──────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct VendorOrderSummary {
    pub id:           Uuid,
    pub status:       String,
    pub total_amount: rust_decimal::Decimal,
    pub delivery_fee: rust_decimal::Decimal,
    pub address:      String,
    pub placed_at:    DateTime<Utc>,
    pub updated_at:   DateTime<Utc>,
    pub item_count:   i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct VendorOrderItem {
    pub id:          Uuid,
    pub book_id:     Uuid,
    pub title:       String,
    pub author:      String,
    pub cover_emoji: Option<String>,
    pub quantity:    i32,
    pub unit_price:  f64,
    pub subtotal:    f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct VendorOrderDetail {
    pub id:           Uuid,
    pub status:       String,
    pub total_amount: f64,
    pub delivery_fee: f64,
    pub address:      String,
    pub notes:        Option<String>,
    pub placed_at:    DateTime<Utc>,
    pub updated_at:   DateTime<Utc>,
    pub items:        Vec<VendorOrderItem>,
}

#[derive(Debug, Serialize, Clone)]
pub struct VendorOrderListResponse {
    pub orders:      Vec<VendorOrderSummary>,
    pub total:       i64,
    pub next_cursor: Option<DateTime<Utc>>,
    pub has_more:    bool,
}

#[derive(Debug, Deserialize)]
pub struct VendorOrderQuery {
    pub status: Option<String>,
    pub limit:  Option<i64>,
    pub cursor: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStatusRequest {
    pub status: String,
}

// ── Inventory models ──────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct InventoryItem {
    pub id:           Uuid,
    pub title:        String,
    pub author:       String,
    pub price:        rust_decimal::Decimal,
    pub cover_emoji:  Option<String>,
    pub cover_color:  Option<String>,
    pub in_stock:     bool,
    pub rating:       rust_decimal::Decimal,
    pub total_reviews: i32,
    pub category_name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateInventoryRequest {
    pub in_stock: Option<bool>,
    pub price:    Option<f64>,
}

// ── Stats model ───────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct VendorStats {
    pub total_orders:   i64,
    pub pending_orders: i64,
    pub active_orders:  i64,  // confirmed + preparing + in_transit
    pub total_revenue:  f64,
    pub total_books:    i64,
    pub low_stock:      i64,  // books with in_stock = false
}