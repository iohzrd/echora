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

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
