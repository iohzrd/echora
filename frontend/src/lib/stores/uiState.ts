import { writable } from 'svelte/store';

export interface UiStateStore {
  showAdminPanel: boolean;
  showPasskeySettings: boolean;
  showProfileModal: boolean;
  profileViewUserId: string | null;
  sidebarOpen: boolean;
  showAddServerDialog: boolean;
  needsServerAuth: boolean;
  tauriAuthIsLogin: boolean;
}

export const uiState = writable<UiStateStore>({
  showAdminPanel: false,
  showPasskeySettings: false,
  showProfileModal: false,
  profileViewUserId: null,
  sidebarOpen: false,
  showAddServerDialog: false,
  needsServerAuth: false,
  tauriAuthIsLogin: true,
});
