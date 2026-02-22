export interface User {
  id: string;
  username: string;
  email: string;
  role: string;
  avatar_url?: string;
  display_name?: string;
}

export interface AuthStateStore {
  user: User | null;
  token: string | null;
}

export const authState = $state<AuthStateStore>({
  user: null,
  token: null,
});
