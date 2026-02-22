export interface UiStateStore {
  showAdminPanel: boolean;
  showPasskeySettings: boolean;
  showProfileModal: boolean;
  profileViewUserId: string | null;
  sidebarOpen: boolean;
  showAddServerDialog: boolean;
  needsServerAuth: boolean;
  tauriAuthIsLogin: boolean;
  tauriVersion: string;
}

export const uiState = $state<UiStateStore>({
  showAdminPanel: false,
  showPasskeySettings: false,
  showProfileModal: false,
  profileViewUserId: null,
  sidebarOpen: false,
  showAddServerDialog: false,
  needsServerAuth: false,
  tauriAuthIsLogin: true,
  tauriVersion: '',
});
