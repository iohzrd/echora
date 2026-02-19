use axum::{
    Json, Router,
    http::{HeaderValue, Method},
    routing::{delete, get, post, put},
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

mod auth;
mod auth_routes;
mod database;
mod link_preview;
mod models;
mod routes;
mod sfu;
mod shared;
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

    let sfu_service = sfu::service::SfuService::new()
        .await
        .expect("Failed to initialize SFU service");

    let state = Arc::new(AppState::new(db, sfu_service));

    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/auth/register", post(auth_routes::register))
        .route("/api/auth/login", post(auth_routes::login))
        .route("/api/auth/me", get(auth_routes::me))
        .route(
            "/api/channels",
            get(routes::get_channels).post(routes::create_channel),
        )
        .route(
            "/api/channels/{channel_id}",
            put(routes::update_channel).delete(routes::delete_channel),
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
        .fallback_service(ServeDir::new("static").fallback(ServeFile::new("static/index.html")))
        .layer(build_cors_layer())
        .with_state(state);

    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();

    info!("Echora backend server starting on http://{}", bind_addr);

    axum::serve(listener, app).await.unwrap();
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
        .allow_headers(tower_http::cors::Any)
        .allow_credentials(false)
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
