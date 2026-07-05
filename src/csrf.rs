use axum::http::HeaderMap;
use axum_csrf::CsrfToken;

use crate::error::AppError;

pub const CSRF_HEADER: &str = "x-csrf-token";

/// Verifies the `X-CSRF-Token` header HTMX attaches to every non-GET request
/// (watari.md §8.4/§12) against the token stored in the CSRF cookie.
pub fn verify(token: &CsrfToken, headers: &HeaderMap) -> Result<(), AppError> {
    let submitted = headers
        .get(CSRF_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("missing CSRF token".into()))?;

    token
        .verify(submitted)
        .map_err(|_| AppError::BadRequest("invalid or expired CSRF token".into()))
}
