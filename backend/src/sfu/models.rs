use mediasoup::prelude::*;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ProducerEntry {
    pub id: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParticipantConnection {
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub transport_id: String,
    pub producers: Vec<ProducerEntry>,
    pub consumer_ids: Vec<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransportOptions {
    pub id: String,
    pub ice_parameters: IceParameters,
    pub ice_candidates: Vec<IceCandidate>,
    pub dtls_parameters: DtlsParameters,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProducerInfo {
    pub producer_id: String,
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub kind: MediaKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConsumerData {
    pub id: String,
    pub producer_id: String,
    pub kind: MediaKind,
    pub rtp_parameters: RtpParameters,
}
