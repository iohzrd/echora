export interface EmojiPickerState {
  messageId: string | null;
  anchorRect: { top: number; bottom: number; left: number; right: number } | null;
  onSelect: ((emoji: string) => void) | null;
}

export const emojiPickerState = $state<EmojiPickerState>({
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
  emojiPickerState.messageId = messageId;
  emojiPickerState.anchorRect = { top: rect.top, bottom: rect.bottom, left: rect.left, right: rect.right };
  emojiPickerState.onSelect = onSelect;
}

export function closeEmojiPicker() {
  emojiPickerState.messageId = null;
  emojiPickerState.anchorRect = null;
  emojiPickerState.onSelect = null;
}
