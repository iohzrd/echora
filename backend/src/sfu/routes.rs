use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use mediasoup::prelude::*;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::models::AppState;
use crate::sfu::models::{ConsumerData, ProducerInfo, TransportOptions};
use crate::shared::{AppError, AppResult};

#[derive(Debug, Deserialize)]
pub struct CreateTransportRequest {
    pub channel_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ConnectTransportRequest {
    pub dtls_parameters: DtlsParameters,
}

#[derive(Debug, Deserialize)]
pub struct ProduceRequest {
    pub kind: MediaKind,
    pub rtp_parameters: RtpParameters,
    pub label: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ProduceResponse {
    pub producer_id: String,
    pub channel_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ConsumeRequest {
    pub producer_id: String,
    pub rtp_capabilities: RtpCapabilities,
}

pub async fn create_transport(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<CreateTransportRequest>,
) -> AppResult<Json<TransportOptions>> {
    let user_id: Uuid = auth_user
        .0
        .sub
        .parse()
        .map_err(|_| AppError::bad_request("Invalid user ID"))?;

    state
        .sfu_service
        .create_transport(req.channel_id, user_id)
        .await
        .map(Json)
        .map_err(|e| {
            tracing::error!("Failed to create transport: {}", e);
            AppError::internal(e.to_string())
        })
}

pub async fn connect_transport(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(transport_id): Path<String>,
    Json(req): Json<ConnectTransportRequest>,
) -> AppResult<StatusCode> {
    state
        .sfu_service
        .connect_transport(&transport_id, req.dtls_parameters)
        .await
        .map(|_| StatusCode::OK)
        .map_err(|e| {
            tracing::error!("Failed to connect transport {}: {}", transport_id, e);
            AppError::internal(e.to_string())
        })
}

pub async fn produce(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(transport_id): Path<String>,
    Json(req): Json<ProduceRequest>,
) -> AppResult<Json<ProduceResponse>> {
    let info = state
        .sfu_service
        .produce(&transport_id, req.kind, req.rtp_parameters, req.label)
        .await
        .map_err(|e| {
            tracing::error!("Failed to produce on transport {}: {}", transport_id, e);
            AppError::internal(e.to_string())
        })?;

    // Broadcast new_producer event so other participants can consume immediately
    let event = serde_json::json!({
        "type": "new_producer",
        "data": {
            "producer_id": info.producer_id,
            "channel_id": info.channel_id,
            "user_id": info.user_id,
            "kind": info.kind,
            "label": info.label,
        }
    });
    let _ = state.global_broadcast.send(event.to_string());

    Ok(Json(ProduceResponse {
        producer_id: info.producer_id,
        channel_id: info.channel_id,
        user_id: info.user_id,
    }))
}

pub async fn consume(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(transport_id): Path<String>,
    Json(req): Json<ConsumeRequest>,
) -> AppResult<Json<ConsumerData>> {
    state
        .sfu_service
        .consume(&transport_id, &req.producer_id, req.rtp_capabilities)
        .await
        .map(Json)
        .map_err(|e| {
            tracing::error!("Failed to consume on transport {}: {}", transport_id, e);
            AppError::internal(e.to_string())
        })
}

pub async fn close_connection(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(transport_id): Path<String>,
) -> AppResult<StatusCode> {
    state
        .sfu_service
        .close_connection(&transport_id)
        .await
        .map(|_| StatusCode::OK)
        .map_err(|e| {
            tracing::error!("Failed to close connection {}: {}", transport_id, e);
            AppError::internal(e.to_string())
        })
}

pub async fn get_channel_producers(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(channel_id): Path<Uuid>,
) -> AppResult<Json<Vec<ProducerInfo>>> {
    Ok(Json(state.sfu_service.get_channel_producers(channel_id)))
}

pub async fn get_router_capabilities(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(channel_id): Path<Uuid>,
) -> AppResult<Json<RtpCapabilitiesFinalized>> {
    state
        .sfu_service
        .get_router_capabilities(channel_id)
        .await
        .map(Json)
        .map_err(|e| {
            tracing::error!(
                "Failed to get router capabilities for channel {}: {}",
                channel_id,
                e
            );
            AppError::internal(e.to_string())
        })
}
