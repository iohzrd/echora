import * as mediasoupClient from 'mediasoup-client';
import AuthService from './auth';
import { API } from './api';
import { getApiBase } from './config';
import { appFetch } from './serverManager';

// WebKitGTK's GStreamer-based WebRTC omits a=ssrc lines from SDP offers, which
// causes mediasoup-client to fail with "no a=ssrc lines found". We inject
// placeholder SSRC lines so mediasoup-client can parse the SDP. The real SSRC
// is read from the RTP sender stats and patched into rtpParameters before the
// produce request reaches the server (see fixProducerSsrc).
if (typeof RTCPeerConnection !== 'undefined') {
  const origCreateOffer = RTCPeerConnection.prototype.createOffer as
    (this: RTCPeerConnection, options?: RTCOfferOptions) => Promise<RTCSessionDescriptionInit>;

  RTCPeerConnection.prototype.createOffer = async function (
    this: RTCPeerConnection,
    ...args: [RTCOfferOptions?]
  ): Promise<RTCSessionDescriptionInit> {
    const offer = await origCreateOffer.apply(this, args);
    if (offer.sdp && !offer.sdp.includes('a=ssrc:')) {
      offer.sdp = offer.sdp.replace(
        /^(m=(?:audio|video)\s.+(?:\r?\n(?!m=).+)*)/gm,
        (section: string) => {
          if (section.includes('a=ssrc:')) return section;
          const ssrc = Math.floor(Math.random() * 0xFFFFFFFF);
          return section + `\r\na=ssrc:${ssrc} cname:webkitgtk\r\na=ssrc:${ssrc} msid:webkitgtk webkitgtk`;
        }
      );
    }
    return offer;
  } as typeof RTCPeerConnection.prototype.createOffer;
}

interface TransportOptions {
  id: string;
  ice_parameters: mediasoupClient.types.IceParameters;
  ice_candidates: mediasoupClient.types.IceCandidate[];
  dtls_parameters: mediasoupClient.types.DtlsParameters;
}

async function createTransportRequest(channelId: string): Promise<TransportOptions> {
  return API.jsonRequest('/webrtc/transport', 'POST', { channel_id: channelId }, 'Failed to create transport');
}

async function connectTransportRequest(
  transportId: string,
  dtlsParameters: mediasoupClient.types.DtlsParameters,
): Promise<void> {
  return API.jsonRequest(
    `/webrtc/transport/${transportId}/connect`,
    'POST',
    { dtls_parameters: dtlsParameters },
    'Failed to connect transport',
  );
}

async function produceRequest(
  transportId: string,
  kind: mediasoupClient.types.MediaKind,
  rtpParameters: mediasoupClient.types.RtpParameters,
  label?: string,
): Promise<string> {
  const body: Record<string, unknown> = {
    kind,
    rtp_parameters: rtpParameters,
  };
  if (label) {
    body.label = label;
  }

  const data = await API.jsonRequest<{ producer_id: string }>(
    `/webrtc/transport/${transportId}/produce`,
    'POST',
    body,
    'Failed to produce',
  );
  return data.producer_id;
}

async function deleteTransportRequest(transportId: string): Promise<void> {
  return API.request(`/webrtc/transport/${transportId}`, { method: 'DELETE' }, 'Failed to close transport');
}

export interface ProducerInfo {
  producer_id: string;
  channel_id: string;
  user_id: string;
  kind: string;
  label?: string;
}

export async function getChannelProducers(channelId: string): Promise<ProducerInfo[]> {
  return API.request(`/webrtc/channel/${channelId}/producers`, {}, 'Failed to get channel producers');
}

export class MediasoupManager {
  private device: mediasoupClient.Device | null = null;
  private sendTransport: mediasoupClient.types.Transport | null = null;
  private recvTransport: mediasoupClient.types.Transport | null = null;
  private producers: Map<string, mediasoupClient.types.Producer> = new Map();
  private screenProducers: mediasoupClient.types.Producer[] = [];
  private cameraProducers: mediasoupClient.types.Producer[] = [];
  private consumers: Map<string, mediasoupClient.types.Consumer> = new Map();
  private consumedProducerIds: Set<string> = new Set();

  public onTrack:
    | ((track: MediaStreamTrack, userId: string, kind: string, label?: string) => void)
    | null = null;

  async init(channelId: string) {
    await this.initDevice(channelId);
    await this.createSendTransport(channelId);
    await this.createRecvTransport(channelId);
  }

  private async initDevice(channelId: string) {
    this.device = new mediasoupClient.Device();

    const routerRtpCapabilities = await API.request<mediasoupClient.types.RtpCapabilities>(
      `/webrtc/channel/${channelId}/router-capabilities`,
      {},
      'Failed to get router capabilities',
    );
    await this.device.load({ routerRtpCapabilities });
  }

  private async createSendTransport(channelId: string) {
    if (!this.device) {
      throw new Error('Device not initialized');
    }

    const transportData = await createTransportRequest(channelId);

    this.sendTransport = this.device.createSendTransport({
      id: transportData.id,
      iceParameters: transportData.ice_parameters,
      iceCandidates: transportData.ice_candidates,
      dtlsParameters: transportData.dtls_parameters,
    });

    this.setupTransportConnectHandler(this.sendTransport, transportData.id);
    this.setupProduceHandler(transportData.id);
  }

  private async createRecvTransport(channelId: string) {
    if (!this.device) {
      throw new Error('Device not initialized');
    }

    const transportData = await createTransportRequest(channelId);

    this.recvTransport = this.device.createRecvTransport({
      id: transportData.id,
      iceParameters: transportData.ice_parameters,
      iceCandidates: transportData.ice_candidates,
      dtlsParameters: transportData.dtls_parameters,
    });

    this.setupTransportConnectHandler(this.recvTransport, transportData.id);
  }

  private setupTransportConnectHandler(transport: mediasoupClient.types.Transport, transportId: string) {
    transport.on(
      'connect',
      async ({ dtlsParameters }, callback, errback) => {
        try {
          await connectTransportRequest(transportId, dtlsParameters);
          callback();
        } catch (error) {
          errback(error as Error);
        }
      },
    );
  }

  private setupProduceHandler(transportId: string) {
    if (!this.sendTransport) return;

    this.sendTransport.on(
      'produce',
      async ({ kind, rtpParameters, appData }, callback, errback) => {
        try {
          // WebKitGTK SSRC fix: the SDP had a placeholder SSRC but GStreamer uses
          // its own. Read the real SSRC from the RTP sender and update rtpParameters
          // before sending to the server, so mediasoup matches the actual RTP stream.
          await this.fixProducerSsrc(kind, rtpParameters);

          const producerId = await produceRequest(
            transportId,
            kind,
            rtpParameters,
            appData?.label as string | undefined,
          );
          callback({ id: producerId });
        } catch (error) {
          errback(error as Error);
        }
      },
    );
  }

  /**
   * Fix SSRC in rtpParameters by reading the real SSRC from the RTP sender.
   * WebKitGTK/GStreamer sends RTP with its own SSRC that differs from the
   * placeholder injected into the SDP. Without this fix, mediasoup drops all
   * packets because the SSRC doesn't match.
   */
  private async fixProducerSsrc(
    kind: mediasoupClient.types.MediaKind,
    rtpParameters: mediasoupClient.types.RtpParameters,
  ): Promise<void> {
    try {
      const pc = (this.sendTransport as unknown as { _handler?: { _pc?: RTCPeerConnection } })
        ?._handler?._pc;
      if (!pc) return;

      // Use the LAST matching sender: when producing screen audio there are
      // multiple audio senders (voice + screen) and the newest one (last) is
      // the one currently being produced.
      const senders = pc.getSenders().filter(s => s.track?.kind === kind);
      const sender = senders[senders.length - 1];
      if (!sender) return;

      let realSsrc: number | null = null;
      for (let attempt = 0; attempt < 10; attempt++) {
        await new Promise(resolve => setTimeout(resolve, 200));
        const stats = await sender.getStats();
        stats.forEach((stat) => {
          if (stat.type === 'outbound-rtp' && stat.ssrc) {
            realSsrc = stat.ssrc;
          }
        });
        if (realSsrc !== null) break;
      }

      if (realSsrc === null) return;

      const oldSsrc = rtpParameters.encodings?.[0]?.ssrc;
      if (oldSsrc === realSsrc) return;

      if (rtpParameters.encodings?.[0]) {
        rtpParameters.encodings[0].ssrc = realSsrc;
      }
    } catch {
      // SSRC fix is best-effort; standard browsers don't need it
    }
  }

  async produce(track: MediaStreamTrack) {
    if (!this.sendTransport) {
      throw new Error('Send transport not initialized');
    }

    const producer = await this.sendTransport.produce({ track });
    this.producers.set(producer.id, producer);
    return producer.id;
  }

  async produceScreen(track: MediaStreamTrack) {
    if (!this.sendTransport) {
      throw new Error('Send transport not initialized');
    }

    const producer = await this.sendTransport.produce({
      track,
      appData: { label: 'screen' },
    });
    this.screenProducers.push(producer);
    this.producers.set(producer.id, producer);
    return producer.id;
  }

  async replaceProducerTrack(newTrack: MediaStreamTrack): Promise<void> {
    for (const producer of this.producers.values()) {
      if (producer.kind === 'audio' && !producer.closed) {
        await producer.replaceTrack({ track: newTrack });
        break;
      }
    }
  }

  closeScreenProducers() {
    for (const producer of this.screenProducers) {
      producer.close();
      this.producers.delete(producer.id);
    }
    this.screenProducers = [];
  }

  async produceCamera(track: MediaStreamTrack) {
    if (!this.sendTransport) {
      throw new Error('Send transport not initialized');
    }

    const producer = await this.sendTransport.produce({
      track,
      appData: { label: 'camera' },
    });
    this.cameraProducers.push(producer);
    this.producers.set(producer.id, producer);
    return producer.id;
  }

  closeCameraProducers() {
    for (const producer of this.cameraProducers) {
      producer.close();
      this.producers.delete(producer.id);
    }
    this.cameraProducers = [];
  }

  async consume(producerId: string, userId: string, label?: string) {
    if (!this.recvTransport || !this.device) {
      throw new Error('Receive transport or device not initialized');
    }

    // Skip if we've already consumed this producer
    if (this.consumedProducerIds.has(producerId)) {
      return null;
    }
    this.consumedProducerIds.add(producerId);

    // consume has custom error handling for STALE_TRANSPORT detection
    const response = await appFetch(
      `${getApiBase()}/webrtc/transport/${this.recvTransport.id}/consume`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...AuthService.getAuthHeaders(),
        },
        body: JSON.stringify({
          producer_id: producerId,
          rtp_capabilities: this.device.rtpCapabilities,
        }),
      },
    );

    if (!response.ok) {
      const errorText = await response.text();
      if (errorText.includes('Transport not found')) {
        throw new Error('STALE_TRANSPORT');
      }
      throw new Error(`Failed to consume producer: ${errorText}`);
    }

    const data = await response.json();

    const consumer = await this.recvTransport.consume({
      id: data.id,
      producerId: data.producer_id,
      kind: data.kind,
      rtpParameters: data.rtp_parameters,
    });

    this.consumers.set(consumer.id, consumer);
    await consumer.resume();

    this.onTrack?.(consumer.track, userId, data.kind, label);

    return consumer;
  }

  pauseAudioProducer() {
    for (const producer of this.producers.values()) {
      if (producer.kind === 'audio' && !producer.paused) {
        producer.pause();
      }
    }
  }

  resumeAudioProducer() {
    for (const producer of this.producers.values()) {
      if (producer.kind === 'audio' && producer.paused) {
        producer.resume();
      }
    }
  }

  setConsumersEnabled(enabled: boolean) {
    for (const consumer of this.consumers.values()) {
      if (consumer.kind === 'audio') {
        if (enabled) {
          consumer.resume();
        } else {
          consumer.pause();
        }
      }
    }
  }

  async close() {
    if (this.sendTransport) {
      try {
        await deleteTransportRequest(this.sendTransport.id);
      } catch (e) {
        console.error('Failed to close send transport:', e);
      }
      this.sendTransport.close();
    }

    if (this.recvTransport) {
      try {
        await deleteTransportRequest(this.recvTransport.id);
      } catch (e) {
        console.error('Failed to close recv transport:', e);
      }
      this.recvTransport.close();
    }

    this.screenProducers = [];
    this.cameraProducers = [];
    this.producers.clear();
    this.consumers.clear();
    this.consumedProducerIds.clear();
    this.device = null;
    this.sendTransport = null;
    this.recvTransport = null;
  }
}
