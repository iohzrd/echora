import { uiState } from '../stores/uiState';

export function openAdminPanel() {
  uiState.update((s) => ({ ...s, showAdminPanel: true }));
}

export function closeAdminPanel() {
  uiState.update((s) => ({ ...s, showAdminPanel: false }));
}

export function openPasskeySettings() {
  uiState.update((s) => ({ ...s, showPasskeySettings: true }));
}

export function closePasskeySettings() {
  uiState.update((s) => ({ ...s, showPasskeySettings: false }));
}

export function openProfileModal() {
  uiState.update((s) => ({ ...s, showProfileModal: true }));
}

export function closeProfileModal() {
  uiState.update((s) => ({ ...s, showProfileModal: false }));
}

export function viewUserProfile(userId: string) {
  uiState.update((s) => ({ ...s, profileViewUserId: userId }));
}

export function closeProfileView() {
  uiState.update((s) => ({ ...s, profileViewUserId: null }));
}

export function toggleSidebar() {
  uiState.update((s) => ({ ...s, sidebarOpen: !s.sidebarOpen }));
}

export function closeSidebar() {
  uiState.update((s) => ({ ...s, sidebarOpen: false }));
}

export function openAddServerDialog() {
  uiState.update((s) => ({ ...s, showAddServerDialog: true }));
}

export function closeAddServerDialog() {
  uiState.update((s) => ({ ...s, showAddServerDialog: false }));
}

export function setNeedsServerAuth(value: boolean) {
  uiState.update((s) => ({ ...s, needsServerAuth: value }));
}

export function setTauriAuthIsLogin(value: boolean) {
  uiState.update((s) => ({ ...s, tauriAuthIsLogin: value }));
}
