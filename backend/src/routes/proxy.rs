use axum::extract::{Query, State};
use futures_util::StreamExt;
use serde::Deserialize;
use std::sync::Arc;

use crate::models::AppState;
use crate::shared::AppError;
use crate::shared::validation::MAX_IMAGE_PROXY_SIZE;

#[derive(Debug, Deserialize)]
pub struct ImageProxyQuery {
    pub url: String,
    pub sig: String,
}

pub async fn proxy_image(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ImageProxyQuery>,
) -> Result<axum::response::Response, AppError> {
    use axum::body::Body;
    use axum::response::Response;
    use base64::Engine;

    let secret = crate::auth::hmac_secret();

    // Decode base64url-encoded URL
    let image_url = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&query.url)
        .map_err(|_| AppError::bad_request("Invalid URL encoding"))?;
    let image_url =
        String::from_utf8(image_url).map_err(|_| AppError::bad_request("Invalid URL encoding"))?;

    // Verify HMAC signature
    if !crate::link_preview::verify_image_signature(&image_url, &query.sig, secret) {
        return Err(AppError::forbidden("Invalid signature"));
    }

    // SSRF protection: validate the URL is safe to fetch
    if !crate::link_preview::is_safe_url(&image_url).await {
        return Err(AppError::bad_request("URL failed safety check"));
    }

    let response = state
        .http_client
        .get(&image_url)
        .send()
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    // Only proxy image content types
    if !content_type.starts_with("image/") {
        return Err(AppError::bad_request("Not an image"));
    }

    // Reject early if Content-Length header exceeds limit
    if let Some(content_length) = response.content_length()
        && content_length as usize > MAX_IMAGE_PROXY_SIZE
    {
        return Err(AppError::bad_request("Image too large"));
    }

    // Stream the body with a hard size cap to avoid buffering unlimited data
    let mut buf = Vec::with_capacity(MAX_IMAGE_PROXY_SIZE.min(256 * 1024));
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::internal(e.to_string()))?;
        let remaining = MAX_IMAGE_PROXY_SIZE.saturating_sub(buf.len());
        if remaining == 0 {
            return Err(AppError::bad_request("Image too large"));
        }
        buf.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }

    Response::builder()
        .header("Content-Type", content_type)
        .header("Content-Length", buf.len())
        .header("Cache-Control", "public, max-age=86400")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(buf))
        .map_err(|e| AppError::internal(e.to_string()))
}
