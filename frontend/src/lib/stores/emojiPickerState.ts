import { writable } from 'svelte/store';

export interface EmojiPickerState {
  messageId: string | null;
  anchorRect: { top: number; bottom: number; left: number; right: number } | null;
  onSelect: ((emoji: string) => void) | null;
}

export const emojiPickerState = writable<EmojiPickerState>({
  messageId: null,
  anchorRect: null,
  onSelect: null,
});

export function openEmojiPicker(
  messageId: string,
  anchorEl: HTMLElement,
  onSelect: (emoji: string) => void,
) {
  const rect = anchorEl.getBoundingClientRect();
  emojiPickerState.set({
    messageId,
    anchorRect: { top: rect.top, bottom: rect.bottom, left: rect.left, right: rect.right },
    onSelect,
  });
}

export function closeEmojiPicker() {
  emojiPickerState.set({ messageId: null, anchorRect: null, onSelect: null });
}
