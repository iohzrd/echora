export function formatTimestamp(timestamp: string): string {
  const date = new Date(timestamp);
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

export function getInitial(name: string): string {
  return name.charAt(0).toUpperCase();
}

export function truncateContent(content: string, maxLen = 100): string {
  if (content.length <= maxLen) return content;
  return content.substring(0, maxLen) + '...';
}
