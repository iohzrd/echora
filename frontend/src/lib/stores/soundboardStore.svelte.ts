import type { SoundboardSound } from "../api";

export interface SoundboardStoreState {
  sounds: SoundboardSound[];
  favorites: string[];
  showPanel: boolean;
}

export const soundboardStore = $state<SoundboardStoreState>({
  sounds: [],
  favorites: [],
  showPanel: false,
});
