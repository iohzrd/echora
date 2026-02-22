use axum::{
    Json, Router,
    http::{HeaderValue, Method},
    routing::{delete, get, post, put},
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

mod admin;
mod auth;
mod auth_routes;
mod database;
mod link_preview;
mod models;
mod passkey_routes;
mod permissions;
mod routes;
mod services;
mod sfu;
mod shared;
mod storage;
mod voice;
mod websocket;

use models::AppState;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let db = shared::db::create_pool()
        .await
        .expect("Failed to create database pool");

    database::seed_data(&db)
        .await
        .expect("Failed to seed database");

    // Pre-populate ban/mute caches from the database so they are accurate
    // on startup without waiting for moderation events.
    let initial_bans = database::get_all_bans(&db)
        .await
        .expect("Failed to load initial bans");
    let initial_mutes = database::get_all_mutes(&db)
        .await
        .expect("Failed to load initial mutes");

    let sfu_service = sfu::service::SfuService::new()
        .await
        .expect("Failed to initialize SFU service");

    let http_client = shared::http::create_http_client(10).expect("Failed to create HTTP client");

    let file_store = storage::build_object_store().expect("Failed to initialize file storage");

    let rp_id = std::env::var("WEBAUTHN_RP_ID").unwrap_or_else(|_| "localhost".to_string());
    let rp_origin = url::Url::parse(
        &std::env::var("WEBAUTHN_RP_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:1420".to_string()),
    )
    .expect("Invalid WEBAUTHN_RP_ORIGIN URL");

    let webauthn = Arc::new(
        webauthn_rs::WebauthnBuilder::new(&rp_id, &rp_origin)
            .expect("Failed to create WebauthnBuilder")
            .rp_name("Echora")
            .build()
            .expect("Failed to build Webauthn"),
    );

    let state = Arc::new(AppState::new(
        db,
        sfu_service,
        http_client,
        file_store,
        webauthn,
    ));

    // Seed in-memory ban/mute caches
    for ban in &initial_bans {
        state.cache_ban(ban.user_id);
    }
    for mute in &initial_mutes {
        state.cache_mute(mute.user_id);
    }

    // Spawn periodic cleanup of expired bans and mutes.
    // Also refreshes the in-memory caches to remove expired entries.
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            let _ = database::cleanup_expired_bans(&cleanup_state.db).await;
            let _ = database::cleanup_expired_mutes(&cleanup_state.db).await;

            // Rebuild caches from DB to evict expired entries
            if let Ok(active_bans) = database::get_all_bans(&cleanup_state.db).await {
                let active_ids: std::collections::HashSet<uuid::Uuid> =
                    active_bans.iter().map(|b| b.user_id).collect();
                cleanup_state
                    .banned_users
                    .retain(|id| active_ids.contains(id));
                for ban in &active_bans {
                    cleanup_state.cache_ban(ban.user_id);
                }
            }
            if let Ok(active_mutes) = database::get_all_mutes(&cleanup_state.db).await {
                let active_ids: std::collections::HashSet<uuid::Uuid> =
                    active_mutes.iter().map(|m| m.user_id).collect();
                cleanup_state
                    .muted_users
                    .retain(|id| active_ids.contains(id));
                for mute in &active_mutes {
                    cleanup_state.cache_mute(mute.user_id);
                }
            }
        }
    });

    // Spawn periodic cleanup of stale WebAuthn challenge states (older than 5 min)
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            let cutoff = std::time::Instant::now() - std::time::Duration::from_secs(300);
            cleanup_state
                .webauthn_reg_state
                .retain(|_, (_, created)| *created > cutoff);
            cleanup_state
                .webauthn_auth_state
                .retain(|_, (_, _, created)| *created > cutoff);
        }
    });

    let auth_routes = Router::new()
        .route("/api/auth/register", post(auth_routes::register))
        .route("/api/auth/login", post(auth_routes::login))
        .route(
            "/api/auth/passkey/login/start",
            post(passkey_routes::start_passkey_auth),
        )
        .route(
            "/api/auth/passkey/login/finish",
            post(passkey_routes::finish_passkey_auth),
        );

    let app = Router::new()
        .route("/api/init", get(routes::get_init))
        .route("/api/health", get(health_check))
        .route(
            "/api/auth/me",
            get(auth_routes::me).put(auth_routes::update_profile),
        )
        .route("/api/auth/password", post(auth_routes::change_password))
        .route(
            "/api/auth/passkey/register/start",
            post(passkey_routes::start_passkey_register),
        )
        .route(
            "/api/auth/passkey/register/finish",
            post(passkey_routes::finish_passkey_register),
        )
        .route("/api/auth/passkeys", get(passkey_routes::list_passkeys))
        .route(
            "/api/auth/passkeys/{passkey_id}",
            delete(passkey_routes::delete_passkey),
        )
        .route(
            "/api/channels",
            get(routes::get_channels).post(routes::create_channel),
        )
        .route(
            "/api/channels/{channel_id}",
            put(routes::update_channel).delete(routes::delete_channel),
        )
        .route(
            "/api/users/{user_id}/profile",
            get(auth_routes::get_user_profile),
        )
        .route("/api/users/{user_id}/avatar", get(auth_routes::get_avatar))
        .route(
            "/api/auth/avatar",
            post(auth_routes::upload_avatar)
                .delete(auth_routes::delete_avatar)
                .layer(RequestBodyLimitLayer::new(2 * 1024 * 1024)), // 2MB for avatar uploads
        )
        .route("/api/users/online", get(routes::get_online_users))
        .route(
            "/api/channels/{channel_id}/messages",
            get(routes::get_messages),
        )
        .route(
            "/api/channels/{channel_id}/messages",
            post(routes::send_message),
        )
        .route(
            "/api/channels/{channel_id}/messages/{message_id}",
            put(routes::edit_message).delete(routes::delete_message),
        )
        .route(
            "/api/channels/{channel_id}/messages/{message_id}/reactions/{emoji}",
            put(routes::add_reaction).delete(routes::remove_reaction),
        )
        .route("/api/voice/join", post(voice::join_voice_channel))
        .route("/api/voice/leave", post(voice::leave_voice_channel))
        .route("/api/voice/states", get(voice::get_all_voice_states))
        .route(
            "/api/voice/channels/{channel_id}/states",
            get(voice::get_voice_states),
        )
        .route("/api/proxy/image", get(routes::proxy_image))
        .route("/ws", get(websocket::websocket_handler))
        // SFU WebRTC routes
        .route("/api/webrtc/transport", post(sfu::routes::create_transport))
        .route(
            "/api/webrtc/transport/{transport_id}/connect",
            post(sfu::routes::connect_transport),
        )
        .route(
            "/api/webrtc/transport/{transport_id}/produce",
            post(sfu::routes::produce),
        )
        .route(
            "/api/webrtc/transport/{transport_id}/consume",
            post(sfu::routes::consume),
        )
        .route(
            "/api/webrtc/transport/{transport_id}",
            delete(sfu::routes::close_connection),
        )
        .route(
            "/api/webrtc/channel/{channel_id}/producers",
            get(sfu::routes::get_channel_producers),
        )
        .route(
            "/api/webrtc/channel/{channel_id}/router-capabilities",
            get(sfu::routes::get_router_capabilities),
        )
        // Admin / moderation routes
        .route("/api/admin/users", get(admin::get_all_users))
        .route(
            "/api/admin/users/{user_id}/role",
            put(admin::change_user_role),
        )
        .route("/api/admin/kick", post(admin::kick_user))
        .route("/api/admin/ban", post(admin::ban_user))
        .route("/api/admin/bans/{user_id}", delete(admin::unban_user))
        .route("/api/admin/bans", get(admin::list_bans))
        .route("/api/admin/mute", post(admin::mute_user))
        .route("/api/admin/mutes/{user_id}", delete(admin::unmute_user))
        .route("/api/admin/mutes", get(admin::list_mutes))
        .route(
            "/api/admin/settings",
            get(admin::get_settings).put(admin::update_setting),
        )
        .route("/api/admin/modlog", get(admin::get_moderation_log))
        // Invite routes
        .route(
            "/api/invites",
            get(admin::list_invites).post(admin::create_invite),
        )
        .route("/api/invites/{invite_id}", delete(admin::revoke_invite))
        .route("/api/invites/{code}/validate", get(admin::validate_invite))
        // Attachment routes
        .route(
            "/api/attachments",
            post(routes::upload_attachment).layer(RequestBodyLimitLayer::new(25 * 1024 * 1024)), // 25MB for uploads
        )
        .route(
            "/api/attachments/{attachment_id}/{filename}",
            get(routes::download_attachment),
        )
        // Custom emoji routes
        .route(
            "/api/custom-emojis",
            get(routes::list_custom_emojis)
                .post(routes::upload_custom_emoji)
                .layer(RequestBodyLimitLayer::new(512 * 1024)), // 512KB for emoji uploads
        )
        .route(
            "/api/custom-emojis/{emoji_id}",
            delete(routes::delete_custom_emoji),
        )
        .route(
            "/api/custom-emojis/{emoji_id}/image",
            get(routes::get_custom_emoji_image),
        )
        // Merge rate-limited auth routes
        .merge(auth_routes)
        .fallback_service(ServeDir::new("static").fallback(ServeFile::new("static/index.html")))
        .layer(RequestBodyLimitLayer::new(1024 * 1024)) // 1MB global body limit
        .layer(build_cors_layer())
        .with_state(state);

    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();

    info!("Echora backend server starting on http://{bind_addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    info!("Server shut down gracefully");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C, shutting down..."),
        _ = terminate => info!("Received SIGTERM, shutting down..."),
    }
}

fn build_cors_layer() -> CorsLayer {
    let allowed_origins = std::env::var("CORS_ORIGINS").unwrap_or_default();

    if allowed_origins.is_empty() {
        info!("CORS: permissive mode (no CORS_ORIGINS set)");
        return CorsLayer::permissive();
    }

    let origins: Vec<HeaderValue> = allowed_origins
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    info!("CORS: restricted to {:?}", origins);

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ])
        .allow_credentials(false)
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
