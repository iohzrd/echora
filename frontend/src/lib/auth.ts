import { getApiBase } from "./config";
import {
  getActiveServer,
  updateServer,
  isTauri,
  appFetch,
} from "./serverManager";
import { authState } from "./stores/authState.svelte";
import type { User } from "./stores/authState.svelte";
export type { User };

export interface AuthResponse {
  token: string;
  user: User;
}

export interface RegisterRequest {
  username: string;
  email: string;
  password: string;
  invite_code?: string;
}

export interface LoginRequest {
  username: string;
  password: string;
}

const TOKEN_KEY = "echocell_token";

class AuthService {
  static async init(): Promise<void> {
    const savedToken = this.getToken();
    if (savedToken) {
      authState.token = savedToken;
      await this.getCurrentUser();
    }
  }

  private static async authRequest(
    endpoint: string,
    data: RegisterRequest | LoginRequest,
    fallbackError: string,
  ): Promise<AuthResponse> {
    const apiBase = getApiBase();
    const response = await appFetch(`${apiBase}/auth/${endpoint}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(data),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(error.error || fallbackError);
    }

    const authResponse: AuthResponse = await response.json();
    this.setAuth(authResponse);
    return authResponse;
  }

  static register(data: RegisterRequest): Promise<AuthResponse> {
    return this.authRequest("register", data, "Registration failed");
  }

  static login(data: LoginRequest): Promise<AuthResponse> {
    return this.authRequest("login", data, "Login failed");
  }

  static async loginWithPasskey(username?: string): Promise<AuthResponse> {
    const { PasskeyService } = await import("./passkey");
    const authResponse = await PasskeyService.loginWithPasskey(username);
    this.setAuth(authResponse);
    return authResponse;
  }

  static async getCurrentUser(): Promise<User | null> {
    const currentToken = this.getToken();
    if (!currentToken) return null;

    try {
      const apiBase = getApiBase();
      const response = await appFetch(`${apiBase}/auth/me`, {
        headers: {
          Authorization: `Bearer ${currentToken}`,
        },
      });

      if (!response.ok) {
        this.logout();
        return null;
      }

      const userData: User = await response.json();
      authState.user = userData;
      return userData;
    } catch {
      // Network error -- do NOT logout; the token may still be valid.
      return null;
    }
  }

  static logout() {
    if (isTauri) {
      const server = getActiveServer();
      if (server) {
        updateServer(server.id, {
          token: undefined,
          userId: undefined,
          username: undefined,
        });
      }
    } else {
      localStorage.removeItem(TOKEN_KEY);
    }
    authState.token = null;
    authState.user = null;
  }

  static getToken(): string | null {
    if (isTauri) {
      const server = getActiveServer();
      return server?.token ?? null;
    }
    return localStorage.getItem(TOKEN_KEY);
  }

  static getAuthHeaders(): Record<string, string> {
    const currentToken = this.getToken();
    return currentToken ? { Authorization: `Bearer ${currentToken}` } : {};
  }

  static setAuth(authResponse: AuthResponse) {
    if (isTauri) {
      const server = getActiveServer();
      if (server) {
        updateServer(server.id, {
          token: authResponse.token,
          userId: authResponse.user.id,
          username: authResponse.user.username,
        });
      }
    } else {
      localStorage.setItem(TOKEN_KEY, authResponse.token);
    }
    authState.token = authResponse.token;
    authState.user = authResponse.user;
  }

  static setUser(updatedUser: User) {
    authState.user = updatedUser;
  }
}

export default AuthService;

export function isModerator(role: string | undefined): boolean {
  return role === "moderator" || role === "admin" || role === "owner";
}

export function isAdminRole(role: string | undefined): boolean {
  return role === "admin" || role === "owner";
}

export function canDeleteMessage(
  authorId: string,
  currentUserId: string,
  role: string | undefined,
): boolean {
  return authorId === currentUserId || isModerator(role);
}
