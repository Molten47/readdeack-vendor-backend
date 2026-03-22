use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use jsonwebtoken::{decode, DecodingKey, Validation};
use std::env;
use tower_cookies::Cookies;
use uuid::Uuid;

use crate::{errors::AppError, models::{Claims, VendorUser}, AppState};

/// Extracts and validates a vendor JWT from the access_token cookie.
/// Returns 401 if no token, 403 if token is valid but role != vendor/admin.
/// bookstore_id is Option — None means the vendor is approved but hasn't
/// set up their store yet. Handlers that strictly need a bookstore should
/// call .require_bookstore() or check themselves.
pub struct VendorAuth(pub VendorUser);

impl VendorAuth {
    /// Convenience method for handlers that MUST have a bookstore.
    /// Returns a clean 403 if bookstore_id is None.
    pub fn require_bookstore(&self) -> Result<Uuid, AppError> {
        self.0.bookstore_id.ok_or_else(|| {
            AppError::Forbidden(
                "No bookstore set up yet. Complete store setup first.".into(),
            )
        })
    }
}

#[async_trait]
impl FromRequestParts<AppState> for VendorAuth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let cookies = Cookies::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::Unauthorized("Could not read cookies".into()))?;

        let token = cookies
            .get("access_token")
            .map(|c| c.value().to_string())
            .ok_or_else(|| AppError::Unauthorized("No access token".into()))?;

        let secret = env::var("JWT_SECRET")
            .map_err(|_| AppError::TokenError("JWT_SECRET not set".into()))?;

        let token_data = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|e| AppError::Unauthorized(e.to_string()))?;

        let claims = token_data.claims;

        // Role check — reject non-vendors immediately
        if claims.role != "vendor" && claims.role != "admin" {
            return Err(AppError::Forbidden("Vendor access required".into()));
        }

        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AppError::TokenError("Invalid user ID in token".into()))?;

        // Resolve bookstore — None is valid (vendor approved, store not set up yet)
        let bookstore_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM bookstores WHERE owner_id = $1 LIMIT 1"
        )
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?;

        Ok(VendorAuth(VendorUser {
            user_id,
            _username:    claims.username,
            bookstore_id, // None if no store yet — handlers decide what to do
        }))
    }
}