use axum::{
    extract::{Path, Query, State},
    Json,
};
use uuid::Uuid;

use crate::{
    errors::AppError,
    middleware::VendorAuth,
    models::{
        InventoryItem, UpdateInventoryRequest, UpdateStatusRequest,
        VendorOrderDetail, VendorOrderItem, VendorOrderListResponse,
        VendorOrderQuery, VendorOrderSummary, VendorStats,
    },
    AppState,
};

// ── Valid status transitions ──────────────────────────────────────
fn valid_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("pending",    "confirmed")  |
        ("confirmed",  "preparing")  |
        ("preparing",  "in_transit") |
        ("in_transit", "delivered")
    )
}

// ── GET /vendor/stats ─────────────────────────────────────────────
// Special case: works even when vendor has no bookstore yet.
// Returns has_bookstore: false with zeroed stats so the dashboard
// can show the setup prompt instead of an error.

pub async fn get_stats(
    State(state): State<AppState>,
    VendorAuth(vendor): VendorAuth,
) -> Result<Json<serde_json::Value>, AppError> {

    // No bookstore yet — return zeroed stats with flag
    let Some(bookstore_id) = vendor.bookstore_id else {
        return Ok(Json(serde_json::json!({
            "has_bookstore":  false,
            "bookstore_id":   null,
            "total_orders":   0,
            "pending_orders": 0,
            "active_orders":  0,
            "todays_revenue": 0.0,
            "total_revenue":  0.0,
            "total_books":    0,
            "active_books":   0,
            "low_stock":      0,
        })));
    };

    let total_orders: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM orders WHERE bookstore_id = $1"
    )
    .bind(bookstore_id)
    .fetch_one(&state.pool)
    .await?;

    let pending_orders: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM orders WHERE bookstore_id = $1 AND status = 'pending'"
    )
    .bind(bookstore_id)
    .fetch_one(&state.pool)
    .await?;

    let active_orders: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM orders WHERE bookstore_id = $1
         AND status IN ('confirmed', 'preparing', 'in_transit')"
    )
    .bind(bookstore_id)
    .fetch_one(&state.pool)
    .await?;

    let total_revenue: f64 = sqlx::query_scalar::<_, Option<rust_decimal::Decimal>>(
        "SELECT SUM(total_amount) FROM orders
         WHERE bookstore_id = $1 AND status = 'delivered'"
    )
    .bind(bookstore_id)
    .fetch_one(&state.pool)
    .await?
    .unwrap_or_default()
    .to_string()
    .parse()
    .unwrap_or(0.0);

    let todays_revenue: f64 = sqlx::query_scalar::<_, Option<rust_decimal::Decimal>>(
        "SELECT SUM(total_amount) FROM orders
         WHERE bookstore_id = $1
           AND status = 'delivered'
           AND placed_at >= CURRENT_DATE"
    )
    .bind(bookstore_id)
    .fetch_one(&state.pool)
    .await?
    .unwrap_or_default()
    .to_string()
    .parse()
    .unwrap_or(0.0);

    let total_books: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM books WHERE bookstore_id = $1"
    )
    .bind(bookstore_id)
    .fetch_one(&state.pool)
    .await?;

    let active_books: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM books WHERE bookstore_id = $1 AND in_stock = true"
    )
    .bind(bookstore_id)
    .fetch_one(&state.pool)
    .await?;

    let low_stock: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM books WHERE bookstore_id = $1 AND in_stock = false"
    )
    .bind(bookstore_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({
        "has_bookstore":  true,
        "bookstore_id":   bookstore_id,
        "total_orders":   total_orders,
        "pending_orders": pending_orders,
        "active_orders":  active_orders,
        "todays_revenue": todays_revenue,
        "total_revenue":  total_revenue,
        "total_books":    total_books,
        "active_books":   active_books,
        "low_stock":      low_stock,
    })))
}

// ── GET /vendor/orders ────────────────────────────────────────────

pub async fn get_orders(
    State(state): State<AppState>,
    auth: VendorAuth,
    Query(params): Query<VendorOrderQuery>,
) -> Result<Json<VendorOrderListResponse>, AppError> {
    let bookstore_id = auth.require_bookstore()?;
    let limit = params.limit.unwrap_or(20).clamp(1, 50);

    let rows = sqlx::query_as::<_, VendorOrderSummary>(
        r#"
        SELECT
            o.id, o.status, o.total_amount, o.delivery_fee,
            o.address, o.placed_at, o.updated_at,
            COUNT(oi.id) AS item_count
        FROM orders o
        LEFT JOIN order_items oi ON oi.order_id = o.id
        WHERE o.bookstore_id = $1
          AND ($2::text IS NULL OR o.status = $2)
          AND ($3::timestamptz IS NULL OR o.placed_at < $3)
        GROUP BY o.id
        ORDER BY
            CASE o.status
                WHEN 'pending'    THEN 1
                WHEN 'confirmed'  THEN 2
                WHEN 'preparing'  THEN 3
                WHEN 'in_transit' THEN 4
                WHEN 'delivered'  THEN 5
                WHEN 'cancelled'  THEN 6
            END,
            o.placed_at ASC
        LIMIT $4
        "#
    )
    .bind(bookstore_id)
    .bind(&params.status)
    .bind(params.cursor)
    .bind(limit + 1)
    .fetch_all(&state.pool)
    .await?;

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM orders WHERE bookstore_id = $1
         AND ($2::text IS NULL OR status = $2)"
    )
    .bind(bookstore_id)
    .bind(&params.status)
    .fetch_one(&state.pool)
    .await?;

    let has_more    = rows.len() as i64 > limit;
    let page        = if has_more { &rows[..limit as usize] } else { &rows[..] };
    let next_cursor = if has_more { page.last().map(|r| r.placed_at) } else { None };

    Ok(Json(VendorOrderListResponse {
        orders: page.to_vec(),
        total,
        next_cursor,
        has_more,
    }))
}

// ── GET /vendor/orders/:id ────────────────────────────────────────

pub async fn get_order(
    State(state): State<AppState>,
    auth: VendorAuth,
    Path(order_id): Path<Uuid>,
) -> Result<Json<VendorOrderDetail>, AppError> {
    let bookstore_id = auth.require_bookstore()?;

    let row = sqlx::query!(
        r#"
        SELECT id, status, total_amount, delivery_fee,
               address, notes, placed_at, updated_at
        FROM orders
        WHERE id = $1 AND bookstore_id = $2
        "#,
        order_id,
        bookstore_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Order not found".into()))?;

    let item_rows = sqlx::query!(
        r#"
        SELECT oi.id, oi.book_id, oi.quantity, oi.unit_price,
               b.title, b.author, b.cover_emoji
        FROM order_items oi
        JOIN books b ON b.id = oi.book_id
        WHERE oi.order_id = $1
        "#,
        order_id
    )
    .fetch_all(&state.pool)
    .await?;

    let items = item_rows
        .into_iter()
        .map(|r| {
            let price = r.unit_price.to_string().parse::<f64>().unwrap_or(0.0);
            VendorOrderItem {
                id:          r.id,
                book_id:     r.book_id,
                title:       r.title,
                author:      r.author,
                cover_emoji: r.cover_emoji,
                quantity:    r.quantity,
                unit_price:  price,
                subtotal:    price * r.quantity as f64,
            }
        })
        .collect();

    Ok(Json(VendorOrderDetail {
        id:           row.id,
        status:       row.status,
        total_amount: row.total_amount.to_string().parse().unwrap_or(0.0),
        delivery_fee: row.delivery_fee.to_string().parse().unwrap_or(0.0),
        address:      row.address,
        notes:        row.notes,
        placed_at:    row.placed_at,
        updated_at:   row.updated_at,
        items,
    }))
}

// ── PATCH /vendor/orders/:id/status ──────────────────────────────

pub async fn update_order_status(
    State(state): State<AppState>,
    auth: VendorAuth,
    Path(order_id): Path<Uuid>,
    Json(body): Json<UpdateStatusRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let bookstore_id = auth.require_bookstore()?;

    let current = sqlx::query_scalar::<_, String>(
        "SELECT status FROM orders WHERE id = $1 AND bookstore_id = $2"
    )
    .bind(order_id)
    .bind(bookstore_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Order not found".into()))?;

    if !valid_transition(&current, &body.status) {
        return Err(AppError::ValidationError(format!(
            "Cannot transition order from '{}' to '{}'", current, body.status
        )));
    }

    sqlx::query(
        "UPDATE orders SET status = $1, updated_at = now() WHERE id = $2"
    )
    .bind(&body.status)
    .bind(order_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({
        "id":      order_id,
        "status":  body.status,
        "message": format!("Order moved to '{}'", body.status)
    })))
}

// ── GET /vendor/inventory ─────────────────────────────────────────

pub async fn get_inventory(
    State(state): State<AppState>,
    auth: VendorAuth,
) -> Result<Json<serde_json::Value>, AppError> {
    let bookstore_id = auth.require_bookstore()?;

    let books = sqlx::query_as::<_, InventoryItem>(
        r#"
        SELECT b.id, b.title, b.author, b.price,
               b.cover_emoji, b.cover_color, b.in_stock,
               b.rating, b.total_reviews,
               c.name AS category_name
        FROM books b
        JOIN categories c ON c.id = b.category_id
        WHERE b.bookstore_id = $1
        ORDER BY b.in_stock DESC, b.title ASC
        "#
    )
    .bind(bookstore_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "books": books })))
}

// ── PATCH /vendor/inventory/:book_id ─────────────────────────────

pub async fn update_inventory(
    State(state): State<AppState>,
    auth: VendorAuth,
    Path(book_id): Path<Uuid>,
    Json(body): Json<UpdateInventoryRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let bookstore_id = auth.require_bookstore()?;

    let result = sqlx::query(
        r#"
        UPDATE books SET
            in_stock = COALESCE($1, in_stock),
            price    = COALESCE($2, price)
        WHERE id = $3 AND bookstore_id = $4
        "#
    )
    .bind(body.in_stock)
    .bind(body.price.map(|p| rust_decimal::Decimal::try_from(p).ok()).flatten())
    .bind(book_id)
    .bind(bookstore_id)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Book not found in your inventory".into()));
    }

    Ok(Json(serde_json::json!({ "updated": true })))
}

// ── PATCH /admin/vendor-applications/:id/approve ─────────────────

pub async fn approve_vendor_application(
    State(state): State<AppState>,
    auth: VendorAuth,
    Path(application_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {

    // Re-check role from DB to prevent privilege escalation
    let role: String = sqlx::query_scalar(
        "SELECT role FROM users WHERE id = $1"
    )
    .bind(auth.0.user_id)
    .fetch_one(&state.pool)
    .await?;

    if role != "admin" {
        return Err(AppError::Forbidden("Admin access required".into()));
    }

    let app = sqlx::query!(
        r#"
        SELECT id, user_id, store_name, store_address, city, status
        FROM vendor_applications
        WHERE id = $1
        "#,
        application_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Application not found".into()))?;

    if app.status != "pending" && app.status != "reviewing" {
        return Err(AppError::ValidationError(
            format!("Application is already '{}'", app.status)
        ));
    }

    let mut tx = state.pool.begin().await?;

    sqlx::query(
        "UPDATE vendor_applications SET status = 'approved', reviewed_at = now()
         WHERE id = $1"
    )
    .bind(application_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE users SET role = 'vendor' WHERE id = $1")
        .bind(app.user_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    // Note: bookstore is created separately via POST /vendor/bookstore
    // in the reader API after the vendor completes the setup wizard.
    Ok(Json(serde_json::json!({
        "approved": true,
        "user_id":  app.user_id,
        "message":  format!("{} is now a vendor — store setup pending", app.store_name)
    })))
}