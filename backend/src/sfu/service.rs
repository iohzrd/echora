use dashmap::DashMap;
use mediasoup::prelude::*;
use mediasoup_types::rtp_parameters::{RtpHeaderExtension, RtpHeaderExtensionDirection};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::sfu::models::{
    ConsumerData, ParticipantConnection, ProducerEntry, ProducerInfo, TransportOptions,
};
use crate::shared::AppError;

const STALE_TRANSPORT_THRESHOLD_SECS: u64 = 5;
const DEFAULT_ANNOUNCED_IP: &str = "127.0.0.1";

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

async fn auto_detect_public_ip() -> Result<String, AppError> {
    tracing::info!("Auto-detecting public IP address...");

    let services = [
        "https://api.ipify.org",
        "https://api64.ipify.org",
        "https://ifconfig.me/ip",
    ];

    let client = reqwest::Client::new();
    for service in services {
        match client.get(service).send().await {
            Ok(response) => {
                if let Ok(ip) = response.text().await {
                    let ip = ip.trim().to_string();
                    tracing::info!("Detected public IP: {} (from {})", ip, service);
                    return Ok(ip);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to query {}: {}", service, e);
                continue;
            }
        }
    }

    Err(AppError::internal(
        "Failed to auto-detect public IP from all services",
    ))
}

async fn get_announced_ip() -> String {
    if let Ok(ip) = std::env::var("MEDIASOUP_ANNOUNCED_IP") {
        tracing::info!("Using announced IP from MEDIASOUP_ANNOUNCED_IP: {}", ip);
        return ip;
    }

    match auto_detect_public_ip().await {
        Ok(ip) => ip,
        Err(e) => {
            tracing::warn!(
                "Failed to auto-detect public IP: {}. Falling back to {}",
                e,
                DEFAULT_ANNOUNCED_IP
            );
            DEFAULT_ANNOUNCED_IP.to_string()
        }
    }
}

pub struct SfuService {
    worker: Worker,
    routers: DashMap<Uuid, Arc<Router>>,
    router_capabilities: DashMap<Uuid, RtpCapabilities>,
    connections: DashMap<String, ParticipantConnection>,
    channel_connections: DashMap<Uuid, Vec<String>>,
    user_connections: DashMap<(Uuid, Uuid), Vec<String>>,
    transports: DashMap<String, Arc<Mutex<WebRtcTransport>>>,
    producers: DashMap<String, Arc<Producer>>,
    consumers: DashMap<String, Arc<Consumer>>,
    announced_ip: String,
}

impl SfuService {
    pub async fn new() -> Result<Self, AppError> {
        let announced_ip = get_announced_ip().await;

        let worker_manager = WorkerManager::new();
        let worker = worker_manager
            .create_worker(WorkerSettings::default())
            .await
            .map_err(|e| AppError::internal(format!("Failed to create mediasoup worker: {e}")))?;

        tracing::info!(
            "mediasoup SFU initialized with announced IP: {}",
            announced_ip
        );

        Ok(Self {
            worker,
            routers: DashMap::new(),
            router_capabilities: DashMap::new(),
            connections: DashMap::new(),
            channel_connections: DashMap::new(),
            user_connections: DashMap::new(),
            transports: DashMap::new(),
            producers: DashMap::new(),
            consumers: DashMap::new(),
            announced_ip,
        })
    }

    /// Verify the authenticated user owns the given transport.
    pub fn verify_transport_owner(
        &self,
        transport_id: &str,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        let conn = self
            .connections
            .get(transport_id)
            .ok_or_else(|| AppError::not_found("Transport not found"))?;
        if conn.user_id != user_id {
            return Err(AppError::forbidden("Transport belongs to another user"));
        }
        Ok(())
    }

    pub async fn get_or_create_router(&self, channel_id: Uuid) -> Result<Arc<Router>, AppError> {
        if let Some(router) = self.routers.get(&channel_id) {
            return Ok(router.clone());
        }

        let media_codecs = crate::sfu::codec::create_default_codecs();

        let unfrozen_capabilities = RtpCapabilities {
            codecs: media_codecs.clone(),
            header_extensions: vec![
                RtpHeaderExtension {
                    kind: MediaKind::Audio,
                    uri: RtpHeaderExtensionUri::Mid,
                    preferred_id: 1,
                    preferred_encrypt: false,
                    direction: RtpHeaderExtensionDirection::SendRecv,
                },
                RtpHeaderExtension {
                    kind: MediaKind::Video,
                    uri: RtpHeaderExtensionUri::Mid,
                    preferred_id: 1,
                    preferred_encrypt: false,
                    direction: RtpHeaderExtensionDirection::SendRecv,
                },
                RtpHeaderExtension {
                    kind: MediaKind::Audio,
                    uri: RtpHeaderExtensionUri::AbsSendTime,
                    preferred_id: 4,
                    preferred_encrypt: false,
                    direction: RtpHeaderExtensionDirection::SendRecv,
                },
                RtpHeaderExtension {
                    kind: MediaKind::Video,
                    uri: RtpHeaderExtensionUri::AbsSendTime,
                    preferred_id: 4,
                    preferred_encrypt: false,
                    direction: RtpHeaderExtensionDirection::SendRecv,
                },
                RtpHeaderExtension {
                    kind: MediaKind::Audio,
                    uri: RtpHeaderExtensionUri::TransportWideCcDraft01,
                    preferred_id: 5,
                    preferred_encrypt: false,
                    direction: RtpHeaderExtensionDirection::RecvOnly,
                },
                RtpHeaderExtension {
                    kind: MediaKind::Video,
                    uri: RtpHeaderExtensionUri::TransportWideCcDraft01,
                    preferred_id: 5,
                    preferred_encrypt: false,
                    direction: RtpHeaderExtensionDirection::SendRecv,
                },
            ],
        };

        self.router_capabilities
            .insert(channel_id, unfrozen_capabilities);

        let router = self
            .worker
            .create_router(RouterOptions::new(media_codecs))
            .await
            .map_err(|e| AppError::internal(format!("Failed to create router: {e}")))?;

        let router_arc = Arc::new(router);
        self.routers.insert(channel_id, router_arc.clone());

        Ok(router_arc)
    }

    pub async fn create_transport(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
    ) -> Result<TransportOptions, AppError> {
        use std::net::{IpAddr, Ipv4Addr};

        self.cleanup_stale_transports(channel_id, user_id).await;

        let router = self.get_or_create_router(channel_id).await?;

        let listen_info = ListenInfo {
            protocol: Protocol::Udp,
            ip: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            announced_address: Some(self.announced_ip.clone()),
            port: None,
            port_range: None,
            flags: None,
            send_buffer_size: None,
            recv_buffer_size: None,
            expose_internal_ip: false,
        };

        let transport_options =
            WebRtcTransportOptions::new(WebRtcTransportListenInfos::new(listen_info));

        let transport = router
            .create_webrtc_transport(transport_options)
            .await
            .map_err(|e| AppError::internal(format!("Failed to create WebRTC transport: {e}")))?;

        let transport_id = transport.id().to_string();
        let ice_parameters = transport.ice_parameters().clone();
        let ice_candidates = transport.ice_candidates().clone();
        let dtls_parameters = transport.dtls_parameters();

        tracing::info!(
            "Created transport {} with {} ICE candidates",
            transport_id,
            ice_candidates.len()
        );

        self.transports
            .insert(transport_id.clone(), Arc::new(Mutex::new(transport)));

        let connection = ParticipantConnection {
            channel_id,
            user_id,
            transport_id: transport_id.clone(),
            producers: Vec::new(),
            consumer_ids: Vec::new(),
            created_at: current_timestamp(),
        };
        self.connections.insert(transport_id.clone(), connection);

        self.channel_connections
            .entry(channel_id)
            .or_default()
            .push(transport_id.clone());
        self.user_connections
            .entry((channel_id, user_id))
            .or_default()
            .push(transport_id.clone());

        Ok(TransportOptions {
            id: transport_id,
            ice_parameters,
            ice_candidates,
            dtls_parameters,
        })
    }

    pub async fn connect_transport(
        &self,
        transport_id: &str,
        dtls_parameters: DtlsParameters,
    ) -> Result<(), AppError> {
        let transport = self
            .transports
            .get(transport_id)
            .ok_or_else(|| AppError::not_found("Transport not found"))?
            .clone();

        let transport_guard = transport.lock().await;
        let remote_parameters = WebRtcTransportRemoteParameters { dtls_parameters };
        transport_guard
            .connect(remote_parameters)
            .await
            .map_err(|e| AppError::internal(format!("Failed to connect transport: {e}")))?;

        tracing::info!("Transport {} connected", transport_id);
        Ok(())
    }

    pub async fn produce(
        &self,
        transport_id: &str,
        kind: MediaKind,
        rtp_parameters: RtpParameters,
        label: Option<String>,
    ) -> Result<ProducerInfo, AppError> {
        let transport = self
            .transports
            .get(transport_id)
            .ok_or_else(|| AppError::not_found("Transport not found"))?
            .clone();

        let producer = transport
            .lock()
            .await
            .produce(ProducerOptions::new(kind, rtp_parameters))
            .await
            .map_err(|e| AppError::internal(format!("Failed to produce: {e}")))?;

        let producer_id = producer.id().to_string();
        tracing::info!(
            "Producer created: {} for {:?} (label: {:?})",
            producer_id,
            kind,
            label
        );

        self.producers
            .insert(producer_id.clone(), Arc::new(producer));

        let mut channel_id = None;
        let mut user_id = None;
        if let Some(mut conn) = self.connections.get_mut(transport_id) {
            conn.producers.push(ProducerEntry {
                id: producer_id.clone(),
                label: label.clone(),
            });
            channel_id = Some(conn.channel_id);
            user_id = Some(conn.user_id);
        }

        Ok(ProducerInfo {
            producer_id,
            channel_id: channel_id.ok_or_else(|| AppError::not_found("Connection not found"))?,
            user_id: user_id.ok_or_else(|| AppError::not_found("Connection not found"))?,
            kind,
            label,
        })
    }

    pub async fn consume(
        &self,
        transport_id: &str,
        producer_id: &str,
        rtp_capabilities: RtpCapabilities,
    ) -> Result<ConsumerData, AppError> {
        let transport = self
            .transports
            .get(transport_id)
            .ok_or_else(|| AppError::not_found("Transport not found"))?
            .clone();

        let producer = self
            .producers
            .get(producer_id)
            .ok_or_else(|| AppError::not_found("Producer not found"))?
            .clone();

        let mut consumer_options =
            mediasoup::consumer::ConsumerOptions::new(producer.id(), rtp_capabilities);
        consumer_options.paused = false;

        let consumer = transport
            .lock()
            .await
            .consume(consumer_options)
            .await
            .map_err(|e| AppError::internal(format!("Failed to consume: {e}")))?;

        let consumer_id = consumer.id().to_string();
        let kind = consumer.kind();
        let rtp_parameters = consumer.rtp_parameters().clone();

        tracing::info!("Consumer created: {} for {:?}", consumer_id, kind);

        self.consumers
            .insert(consumer_id.clone(), Arc::new(consumer));

        if let Some(mut conn) = self.connections.get_mut(transport_id) {
            conn.consumer_ids.push(consumer_id.clone());
        }

        Ok(ConsumerData {
            id: consumer_id,
            producer_id: producer_id.to_string(),
            kind,
            rtp_parameters,
        })
    }

    pub fn get_channel_producers(&self, channel_id: Uuid) -> Vec<ProducerInfo> {
        let mut result = Vec::new();

        if let Some(transport_ids) = self.channel_connections.get(&channel_id) {
            for transport_id in transport_ids.value() {
                if let Some(conn) = self.connections.get(transport_id) {
                    for entry in &conn.producers {
                        if let Some(producer) = self.producers.get(&entry.id) {
                            result.push(ProducerInfo {
                                producer_id: entry.id.clone(),
                                channel_id: conn.channel_id,
                                user_id: conn.user_id,
                                kind: producer.kind(),
                                label: entry.label.clone(),
                            });
                        }
                    }
                }
            }
        }

        result
    }

    pub async fn get_router_capabilities(
        &self,
        channel_id: Uuid,
    ) -> Result<RtpCapabilitiesFinalized, AppError> {
        let router = self.get_or_create_router(channel_id).await?;
        Ok(router.rtp_capabilities().clone())
    }

    async fn cleanup_stale_transports(&self, channel_id: Uuid, user_id: Uuid) {
        let now = current_timestamp();
        let stale_transport_ids: Vec<String> = self
            .connections
            .iter()
            .filter(|conn| {
                conn.channel_id == channel_id
                    && conn.user_id == user_id
                    && now - conn.created_at > STALE_TRANSPORT_THRESHOLD_SECS
            })
            .map(|conn| conn.transport_id.clone())
            .collect();

        for transport_id in stale_transport_ids {
            tracing::info!(
                "Cleaning up stale transport {} for user {} in channel {}",
                transport_id,
                user_id,
                channel_id
            );
            let _ = self.close_connection(&transport_id).await;
        }
    }

    pub async fn close_connection(&self, transport_id: &str) -> Result<(Uuid, Uuid), AppError> {
        let connection = self
            .connections
            .remove(transport_id)
            .ok_or_else(|| AppError::not_found("Connection not found"))?
            .1;

        let channel_id = connection.channel_id;
        let user_id = connection.user_id;

        for entry in connection.producers {
            self.producers.remove(&entry.id);
        }

        for consumer_id in connection.consumer_ids {
            self.consumers.remove(&consumer_id);
        }

        self.transports.remove(transport_id);

        if let Some(mut transport_ids) = self.channel_connections.get_mut(&channel_id) {
            transport_ids.retain(|id| id != transport_id);
        }
        if let Some(mut transport_ids) = self.user_connections.get_mut(&(channel_id, user_id)) {
            transport_ids.retain(|id| id != transport_id);
        }

        Ok((channel_id, user_id))
    }

    pub async fn close_user_connections(&self, channel_id: Uuid, user_id: Uuid) {
        let transport_ids: Vec<String> = self
            .user_connections
            .get(&(channel_id, user_id))
            .map(|ids| ids.value().clone())
            .unwrap_or_default();

        for transport_id in transport_ids {
            let _ = self.close_connection(&transport_id).await;
        }
    }
}
