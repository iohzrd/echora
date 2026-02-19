import * as mediasoupClient from 'mediasoup-client';
import AuthService from './auth';

const API_BASE = import.meta.env.VITE_API_BASE || '/api';

interface TransportOptions {
  id: string;
  ice_parameters: mediasoupClient.types.IceParameters;
  ice_candidates: mediasoupClient.types.IceCandidate[];
  dtls_parameters: mediasoupClient.types.DtlsParameters;
}

async function createTransportRequest(
  channelId: string,
): Promise<TransportOptions> {
  const response = await fetch(`${API_BASE}/webrtc/transport`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...AuthService.getAuthHeaders(),
    },
    body: JSON.stringify({ channel_id: channelId }),
  });

  if (!response.ok) {
    throw new Error(`Failed to create transport: ${await response.text()}`);
  }

  return await response.json();
}

async function connectTransportRequest(
  transportId: string,
  dtlsParameters: mediasoupClient.types.DtlsParameters,
): Promise<void> {
  const response = await fetch(
    `${API_BASE}/webrtc/transport/${transportId}/connect`,
    {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...AuthService.getAuthHeaders(),
      },
      body: JSON.stringify({ dtls_parameters: dtlsParameters }),
    },
  );

  if (!response.ok) {
    throw new Error(`Failed to connect transport: ${await response.text()}`);
  }
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

  const response = await fetch(
    `${API_BASE}/webrtc/transport/${transportId}/produce`,
    {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...AuthService.getAuthHeaders(),
      },
      body: JSON.stringify(body),
    },
  );

  if (!response.ok) {
    throw new Error(`Failed to produce: ${await response.text()}`);
  }

  const data = await response.json();
  return data.producer_id;
}

async function deleteTransportRequest(transportId: string): Promise<void> {
  await fetch(`${API_BASE}/webrtc/transport/${transportId}`, {
    method: 'DELETE',
    headers: AuthService.getAuthHeaders(),
  });
}

export interface ProducerInfo {
  producer_id: string;
  channel_id: string;
  user_id: string;
  kind: string;
  label?: string;
}

export async function getChannelProducers(channelId: string): Promise<ProducerInfo[]> {
  const response = await fetch(`${API_BASE}/webrtc/channel/${channelId}/producers`, {
    headers: AuthService.getAuthHeaders(),
  });
  if (!response.ok) {
    throw new Error(`Failed to get channel producers: ${await response.text()}`);
  }
  return await response.json();
}

export class MediasoupManager {
  private device: mediasoupClient.Device | null = null;
  private sendTransport: mediasoupClient.types.Transport | null = null;
  private recvTransport: mediasoupClient.types.Transport | null = null;
  private producers: Map<string, mediasoupClient.types.Producer> = new Map();
  private screenProducers: mediasoupClient.types.Producer[] = [];
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

    const response = await fetch(
      `${API_BASE}/webrtc/channel/${channelId}/router-capabilities`,
      {
        headers: AuthService.getAuthHeaders(),
      },
    );

    if (!response.ok) {
      throw new Error('Failed to get router capabilities');
    }

    const routerRtpCapabilities = await response.json();
    console.log('Router RTP capabilities:', routerRtpCapabilities);
    await this.device.load({ routerRtpCapabilities });
  }

  private async createSendTransport(channelId: string) {
    if (!this.device) {
      throw new Error('Device not initialized');
    }

    const transportData = await createTransportRequest(channelId);
    console.log(
      'Send transport ICE candidates:',
      JSON.stringify(transportData.ice_candidates, null, 2),
    );

    this.sendTransport = this.device.createSendTransport({
      id: transportData.id,
      iceParameters: transportData.ice_parameters,
      iceCandidates: transportData.ice_candidates,
      dtlsParameters: transportData.dtls_parameters,
    });

    this.setupSendTransportHandlers(transportData.id);
  }

  private setupSendTransportHandlers(transportId: string) {
    if (!this.sendTransport) return;

    this.sendTransport.on(
      'connect',
      async ({ dtlsParameters }, callback, errback) => {
        try {
          console.log('Send transport connecting...');
          await connectTransportRequest(transportId, dtlsParameters);
          console.log('Send transport connected');
          callback();
        } catch (error) {
          console.error('Send transport connect failed:', error);
          errback(error as Error);
        }
      },
    );

    this.sendTransport.on('connectionstatechange', (state) => {
      console.log('Send transport state:', state);
    });

    this.sendTransport.on(
      'produce',
      async ({ kind, rtpParameters, appData }, callback, errback) => {
        try {
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

  private async createRecvTransport(channelId: string) {
    if (!this.device) {
      throw new Error('Device not initialized');
    }

    const transportData = await createTransportRequest(channelId);
    console.log(
      'Recv transport ICE candidates:',
      JSON.stringify(transportData.ice_candidates, null, 2),
    );

    this.recvTransport = this.device.createRecvTransport({
      id: transportData.id,
      iceParameters: transportData.ice_parameters,
      iceCandidates: transportData.ice_candidates,
      dtlsParameters: transportData.dtls_parameters,
    });

    this.setupRecvTransportHandlers(transportData.id);
  }

  private setupRecvTransportHandlers(transportId: string) {
    if (!this.recvTransport) return;

    this.recvTransport.on(
      'connect',
      async ({ dtlsParameters }, callback, errback) => {
        try {
          console.log('Recv transport connecting...');
          await connectTransportRequest(transportId, dtlsParameters);
          console.log('Recv transport connected');
          callback();
        } catch (error) {
          console.error('Recv transport connect failed:', error);
          errback(error as Error);
        }
      },
    );

    this.recvTransport.on('connectionstatechange', (state) => {
      console.log('Recv transport state:', state);
    });
  }

  async produce(track: MediaStreamTrack) {
    if (!this.sendTransport) {
      throw new Error('Send transport not initialized');
    }

    const producer = await this.sendTransport.produce({ track });
    this.producers.set(producer.id, producer);
    console.log('Produced', track.kind, 'track:', producer.id);
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
    console.log('Produced screen', track.kind, 'track:', producer.id);
    return producer.id;
  }

  closeScreenProducers() {
    for (const producer of this.screenProducers) {
      producer.close();
      this.producers.delete(producer.id);
    }
    this.screenProducers = [];
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

    const response = await fetch(
      `${API_BASE}/webrtc/transport/${this.recvTransport.id}/consume`,
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

    console.log('Consumed', data.kind, 'track from user', userId.substring(0, 8), label ? `(${label})` : '');

    if (this.onTrack) {
      this.onTrack(consumer.track, userId, data.kind, label);
    }

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
    this.producers.clear();
    this.consumers.clear();
    this.consumedProducerIds.clear();
    this.device = null;
    this.sendTransport = null;
    this.recvTransport = null;
  }
}
