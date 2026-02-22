import { uiState } from '../stores/uiState.svelte';

export function openAdminPanel() {
  uiState.showAdminPanel = true;
}

export function closeAdminPanel() {
  uiState.showAdminPanel = false;
}

export function openPasskeySettings() {
  uiState.showPasskeySettings = true;
}

export function closePasskeySettings() {
  uiState.showPasskeySettings = false;
}

export function openProfileModal() {
  uiState.showProfileModal = true;
}

export function closeProfileModal() {
  uiState.showProfileModal = false;
}

export function viewUserProfile(userId: string) {
  uiState.profileViewUserId = userId;
}

export function closeProfileView() {
  uiState.profileViewUserId = null;
}

export function toggleSidebar() {
  uiState.sidebarOpen = !uiState.sidebarOpen;
}

export function closeSidebar() {
  uiState.sidebarOpen = false;
}

export function openAddServerDialog() {
  uiState.showAddServerDialog = true;
}

export function closeAddServerDialog() {
  uiState.showAddServerDialog = false;
}

export function setNeedsServerAuth(value: boolean) {
  uiState.needsServerAuth = value;
}

export function setTauriAuthIsLogin(value: boolean) {
  uiState.tauriAuthIsLogin = value;
}
