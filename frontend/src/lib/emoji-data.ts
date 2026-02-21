import { getEmojis } from "unicode-emoji";

export interface EmojiEntry {
  emoji: string;
  description: string;
  keywords: string[];
}

export interface EmojiCategory {
  name: string;
  icon: string;
  emojis: EmojiEntry[];
}

const GROUP_LABELS: Record<string, string> = {
  "smileys-emotion": "Smileys",
  "people-body": "People",
  "animals-nature": "Nature",
  "food-drink": "Food",
  "travel-places": "Travel",
  activities: "Activities",
  objects: "Objects",
  symbols: "Symbols",
  flags: "Flags",
};

const allEmojis = getEmojis();

const grouped = new Map<string, EmojiEntry[]>();
for (const e of allEmojis) {
  const group = e.group;
  if (!grouped.has(group)) {
    grouped.set(group, []);
  }
  grouped.get(group)!.push({
    emoji: e.emoji,
    description: e.description,
    keywords: e.keywords,
  });
}

export const EMOJI_CATEGORIES: EmojiCategory[] = [];
for (const [group, emojis] of grouped) {
  EMOJI_CATEGORIES.push({
    name: GROUP_LABELS[group] || group,
    icon: emojis[0].emoji,
    emojis,
  });
}
