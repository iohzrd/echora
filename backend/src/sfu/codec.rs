use mediasoup::prelude::*;
use std::num::{NonZeroU8, NonZeroU32};

pub const OPUS_CLOCK_RATE: u32 = 48000;
pub const OPUS_CHANNELS: u8 = 2;
pub const VP8_CLOCK_RATE: u32 = 90000;

pub fn create_default_codecs() -> Vec<RtpCodecCapability> {
    vec![
        RtpCodecCapability::Audio {
            mime_type: MimeTypeAudio::Opus,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(OPUS_CLOCK_RATE).unwrap(),
            channels: NonZeroU8::new(OPUS_CHANNELS).unwrap(),
            parameters: RtpCodecParametersParameters::default(),
            rtcp_feedback: vec![RtcpFeedback::TransportCc],
        },
        RtpCodecCapability::Video {
            mime_type: MimeTypeVideo::Vp8,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(VP8_CLOCK_RATE).unwrap(),
            parameters: RtpCodecParametersParameters::from([(
                "x-google-start-bitrate".to_string(),
                1000_u32.into(),
            )]),
            rtcp_feedback: vec![
                RtcpFeedback::Nack,
                RtcpFeedback::NackPli,
                RtcpFeedback::CcmFir,
                RtcpFeedback::GoogRemb,
                RtcpFeedback::TransportCc,
            ],
        },
    ]
}
