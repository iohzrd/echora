import { get } from "svelte/store";
import { voiceManager } from "../voice";
import { voiceStore } from "../stores/voiceStore";
import { getChannelProducers } from "../mediasoup";

export function joinVoice(channelId: string) {
  voiceManager.joinVoiceChannel(channelId).catch((error) => {
    console.error("Failed to join voice channel:", error);
  });
}

export function leaveVoice() {
  voiceManager.leaveVoiceChannel().catch((error) => {
    console.error("Failed to leave voice channel:", error);
  });
}

export function toggleMute() {
  voiceManager.toggleMute().catch((error) => {
    console.error("Failed to toggle mute:", error);
  });
}

export function toggleDeafen() {
  voiceManager.toggleDeafen().catch((error) => {
    console.error("Failed to toggle deafen:", error);
  });
}

let _togglingScreen = false;
export async function toggleScreenShare() {
  if (_togglingScreen) return;
  _togglingScreen = true;
  try {
    const { isScreenSharing } = get(voiceStore);
    if (isScreenSharing) {
      await voiceManager.stopScreenShare();
    } else {
      await voiceManager.startScreenShare();
    }
  } catch (error) {
    if (error instanceof Error && error.name === "NotAllowedError") return;
    console.error("Failed to toggle screen share:", error);
  } finally {
    _togglingScreen = false;
  }
}

let _togglingCamera = false;
export async function toggleCamera() {
  if (_togglingCamera) return;
  _togglingCamera = true;
  try {
    const { isCameraSharing } = get(voiceStore);
    if (isCameraSharing) {
      await voiceManager.stopCamera();
    } else {
      await voiceManager.startCamera();
    }
  } catch (error) {
    if (error instanceof Error && error.name === "NotAllowedError") return;
    console.error("Failed to toggle camera:", error);
  } finally {
    _togglingCamera = false;
  }
}

export async function watchScreen(userId: string, username: string) {
  const { currentVoiceChannel } = get(voiceStore);
  if (!currentVoiceChannel) return;
  voiceStore.update((s) => ({
    ...s,
    watchingScreenUserId: userId,
    watchingScreenUsername: username,
  }));
  try {
    const producers = await getChannelProducers(currentVoiceChannel);
    if (get(voiceStore).currentVoiceChannel !== currentVoiceChannel) return;
    for (const producer of producers) {
      if (producer.user_id === userId && producer.label === "screen") {
        await voiceManager.consumeProducer(
          producer.producer_id,
          userId,
          producer.label,
        );
      }
    }
  } catch (error) {
    console.error("Failed to consume screen share producer:", error);
  }
}

export function stopWatching() {
  voiceStore.update((s) => ({
    ...s,
    watchingScreenUserId: null,
    watchingScreenUsername: "",
  }));
}

export async function watchCamera(userId: string, username: string) {
  const { currentVoiceChannel } = get(voiceStore);
  if (!currentVoiceChannel) return;
  voiceStore.update((s) => ({
    ...s,
    watchingCameraUserId: userId,
    watchingCameraUsername: username,
  }));
  try {
    const producers = await getChannelProducers(currentVoiceChannel);
    if (get(voiceStore).currentVoiceChannel !== currentVoiceChannel) return;
    for (const producer of producers) {
      if (producer.user_id === userId && producer.label === "camera") {
        await voiceManager.consumeProducer(
          producer.producer_id,
          userId,
          producer.label,
        );
      }
    }
  } catch (error) {
    console.error("Failed to consume camera producer:", error);
  }
}

export function stopWatchingCamera() {
  voiceStore.update((s) => ({
    ...s,
    watchingCameraUserId: null,
    watchingCameraUsername: "",
  }));
}

export function getUserVolume(userId: string): number {
  return voiceManager.getUserVolume(userId);
}
