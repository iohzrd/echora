use hmac::{Hmac, Mac};
use linkify::{LinkFinder, LinkKind};
use scraper::{Html, Selector};
use sha2::Sha256;
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

use crate::database;
use crate::models::AppState;

type HmacSha256 = Hmac<Sha256>;

const MAX_BODY_SIZE: usize = 256 * 1024; // 256KB
const MAX_URLS_PER_MESSAGE: usize = 5;

#[derive(Debug, Clone)]
pub struct LinkPreviewData {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub site_name: Option<String>,
}

/// Extract URLs from message content using linkify
pub fn extract_urls(content: &str) -> Vec<String> {
    let mut finder = LinkFinder::new();
    finder.kinds(&[LinkKind::Url]);

    finder
        .links(content)
        .map(|link| link.as_str().to_string())
        .take(MAX_URLS_PER_MESSAGE)
        .collect()
}

/// Check if a URL scheme is safe (http/https only)
pub fn is_safe_scheme(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// Check if an IP address is private/reserved (SSRF protection)
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_loopback()
                || ipv4.is_private()
                || ipv4.is_link_local()
                || ipv4.is_broadcast()
                || ipv4.is_unspecified()
                // Cloud metadata endpoint
                || ipv4.octets() == [169, 254, 169, 254]
                // CGNAT range
                || (ipv4.octets()[0] == 100
                    && ipv4.octets()[1] >= 64
                    && ipv4.octets()[1] <= 127)
        }
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                // IPv4-mapped IPv6
                || ipv6.to_ipv4_mapped().is_some_and(|v4| {
                    v4.is_loopback()
                        || v4.is_private()
                        || v4.is_link_local()
                        || v4.octets() == [169, 254, 169, 254]
                })
        }
    }
}

/// Validate a URL is safe to fetch (SSRF protection)
pub async fn is_safe_url(url: &str) -> bool {
    if !is_safe_scheme(url) {
        return false;
    }

    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return false,
    };

    let host = match parsed.host_str() {
        Some(h) => h,
        None => return false,
    };

    // Try to parse as IP directly
    if let Ok(ip) = host.parse::<IpAddr>() {
        return !is_private_ip(ip);
    }

    // Resolve hostname and check all resolved IPs
    let port = parsed.port_or_known_default().unwrap_or(443);
    let addr = format!("{host}:{port}");
    match tokio::net::lookup_host(&addr).await {
        Ok(addrs) => {
            let addrs: Vec<_> = addrs.collect();
            if addrs.is_empty() {
                return false;
            }
            addrs.iter().all(|addr| !is_private_ip(addr.ip()))
        }
        Err(_) => false,
    }
}

/// Fetch a URL and parse OpenGraph/meta tags
async fn fetch_preview(client: &reqwest::Client, url: &str) -> Result<LinkPreviewData, String> {
    if !is_safe_url(url).await {
        return Err("URL failed safety check".to_string());
    }

    let response = client
        .get(url)
        .header("Accept", "text/html")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // Direct image URLs: create a preview with the image itself
    if content_type.starts_with("image/") {
        let site_name = url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()));
        return Ok(LinkPreviewData {
            url: url.to_string(),
            title: None,
            description: None,
            image_url: Some(url.to_string()),
            site_name,
        });
    }

    if !content_type.contains("text/html") {
        return Err("Not HTML content".to_string());
    }

    // Reject early if Content-Length header exceeds limit
    if let Some(content_length) = response.content_length()
        && content_length as usize > MAX_BODY_SIZE
    {
        return Err("Content too large".to_string());
    }

    // Stream the body with a hard size cap to avoid buffering huge responses
    let mut buf = Vec::with_capacity(MAX_BODY_SIZE.min(64 * 1024));
    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e: reqwest::Error| e.to_string())?;
        let remaining = MAX_BODY_SIZE.saturating_sub(buf.len());
        if remaining == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }

    let html = String::from_utf8_lossy(&buf).into_owned();
    Ok(parse_og_tags(&html, url))
}

/// Cached CSS selectors for OG tag parsing (parsed once, reused across calls)
struct OgSelectors {
    og_title: Selector,
    twitter_title: Selector,
    title: Selector,
    og_description: Selector,
    twitter_description: Selector,
    meta_description: Selector,
    og_image: Selector,
    twitter_image: Selector,
    og_site_name: Selector,
}

static OG_SELECTORS: std::sync::LazyLock<OgSelectors> = std::sync::LazyLock::new(|| OgSelectors {
    og_title: Selector::parse("meta[property='og:title']").unwrap(),
    twitter_title: Selector::parse("meta[name='twitter:title']").unwrap(),
    title: Selector::parse("title").unwrap(),
    og_description: Selector::parse("meta[property='og:description']").unwrap(),
    twitter_description: Selector::parse("meta[name='twitter:description']").unwrap(),
    meta_description: Selector::parse("meta[name='description']").unwrap(),
    og_image: Selector::parse("meta[property='og:image']").unwrap(),
    twitter_image: Selector::parse("meta[name='twitter:image']").unwrap(),
    og_site_name: Selector::parse("meta[property='og:site_name']").unwrap(),
});

/// Parse OpenGraph, Twitter Card, and HTML meta tags from HTML
fn parse_og_tags(html: &str, url: &str) -> LinkPreviewData {
    let document = Html::parse_document(html);
    let sel = &*OG_SELECTORS;

    // Helper to extract meta content attribute from first match
    let meta_content = |selector: &Selector| -> Option<String> {
        document
            .select(selector)
            .next()?
            .value()
            .attr("content")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    // Title: og:title -> twitter:title -> <title>
    let title = meta_content(&sel.og_title)
        .or_else(|| meta_content(&sel.twitter_title))
        .or_else(|| {
            let text = document
                .select(&sel.title)
                .next()?
                .text()
                .collect::<String>();
            let trimmed = text.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });

    // Description: og:description -> twitter:description -> <meta name="description">
    let description = meta_content(&sel.og_description)
        .or_else(|| meta_content(&sel.twitter_description))
        .or_else(|| meta_content(&sel.meta_description))
        .map(|d| crate::shared::truncate_string(&d, 300));

    // Image: og:image -> twitter:image
    let image_url = meta_content(&sel.og_image)
        .or_else(|| meta_content(&sel.twitter_image))
        .and_then(|img| {
            // Resolve relative URLs
            if img.starts_with("http://") || img.starts_with("https://") {
                Some(img)
            } else if let Ok(base) = url::Url::parse(url) {
                base.join(&img).ok().map(|u| u.to_string())
            } else {
                None
            }
        });

    // Site name: og:site_name -> hostname
    let site_name = meta_content(&sel.og_site_name).or_else(|| {
        url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
    });

    LinkPreviewData {
        url: url.to_string(),
        title,
        description,
        image_url,
        site_name,
    }
}

/// Sign an image URL with HMAC-SHA256 for the proxy endpoint
pub fn sign_image_url(image_url: &str, secret: &str) -> (String, String) {
    use base64::Engine;
    let encoded_url = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(image_url);

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(image_url.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());

    (encoded_url, sig)
}

/// Verify an HMAC signature for a proxied image URL (constant-time comparison)
pub fn verify_image_signature(image_url: &str, sig: &str, secret: &str) -> bool {
    let Ok(sig_bytes) = hex::decode(sig) else {
        return false;
    };
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(image_url.as_bytes());
    mac.verify_slice(&sig_bytes).is_ok()
}

/// Spawn an async task to fetch link previews for a message
pub fn spawn_preview_fetch(
    state: Arc<AppState>,
    message_id: Uuid,
    channel_id: Uuid,
    content: String,
) {
    let urls = extract_urls(&content);
    if urls.is_empty() {
        return;
    }

    tokio::spawn(async move {
        let hmac_secret = crate::auth::hmac_secret();

        // Fetch all URLs concurrently
        let fetch_results: Vec<_> = futures_util::future::join_all(urls.iter().map(|url| {
            let client = &state.http_client;
            async move { (url.clone(), fetch_preview(client, url).await) }
        }))
        .await;

        let mut previews = Vec::new();
        for (url, result) in fetch_results {
            match result {
                Ok(mut data) => {
                    // Sign image URL for proxy if present
                    if let Some(ref img_url) = data.image_url {
                        let (encoded, sig) = sign_image_url(img_url, hmac_secret);
                        data.image_url = Some(format!("/api/proxy/image?url={encoded}&sig={sig}"));
                    }

                    // Skip previews with no useful data
                    if data.title.is_none()
                        && data.description.is_none()
                        && data.image_url.is_none()
                    {
                        continue;
                    }

                    match database::upsert_link_preview(&state.db, &data).await {
                        Ok(preview_id) => {
                            if let Err(e) = database::attach_preview_to_message(
                                &state.db, message_id, preview_id,
                            )
                            .await
                            {
                                error!("Failed to attach preview to message: {e}");
                                continue;
                            }
                            previews.push(crate::models::LinkPreview {
                                id: preview_id,
                                url: data.url,
                                title: data.title,
                                description: data.description,
                                image_url: data.image_url,
                                site_name: data.site_name,
                            });
                        }
                        Err(e) => {
                            error!("Failed to save link preview: {e}");
                        }
                    }
                }
                Err(e) => {
                    info!("Failed to fetch preview for {url}: {e}");
                }
            }
        }

        if !previews.is_empty() {
            state.broadcast_channel(
                channel_id,
                "link_preview_ready",
                serde_json::json!({
                    "message_id": message_id,
                    "channel_id": channel_id,
                    "link_previews": previews,
                }),
            );
        }
    });
}
