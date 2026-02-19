import { writable } from 'svelte/store';

const API_BASE = import.meta.env.VITE_API_BASE || '/api';

export interface User {
  id: string;
  username: string;
  email: string;
}

export interface AuthResponse {
  token: string;
  user: User;
}

export interface RegisterRequest {
  username: string;
  email: string;
  password: string;
}

export interface LoginRequest {
  username: string;
  password: string;
}

export const user = writable<User | null>(null);
export const token = writable<string | null>(null);

class AuthService {
  private static tokenKey = 'echora_token';

  static async init(): Promise<void> {
    const savedToken = localStorage.getItem(this.tokenKey);
    if (savedToken) {
      token.set(savedToken);
      await this.getCurrentUser();
    }
  }

  static async register(data: RegisterRequest): Promise<AuthResponse> {
    const response = await fetch(`${API_BASE}/auth/register`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(data),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(error.error || 'Registration failed');
    }

    const authResponse: AuthResponse = await response.json();
    this.setAuth(authResponse);
    return authResponse;
  }

  static async login(data: LoginRequest): Promise<AuthResponse> {
    const response = await fetch(`${API_BASE}/auth/login`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(data),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(error.error || 'Login failed');
    }

    const authResponse: AuthResponse = await response.json();
    this.setAuth(authResponse);
    return authResponse;
  }

  static async getCurrentUser(): Promise<User | null> {
    const currentToken = this.getToken();
    if (!currentToken) return null;

    try {
      const response = await fetch(`${API_BASE}/auth/me`, {
        headers: {
          'Authorization': `Bearer ${currentToken}`,
        },
      });

      if (!response.ok) {
        this.logout();
        return null;
      }

      const userData: User = await response.json();
      user.set(userData);
      return userData;
    } catch (error) {
      this.logout();
      return null;
    }
  }

  static logout() {
    localStorage.removeItem(this.tokenKey);
    token.set(null);
    user.set(null);
  }

  static getToken(): string | null {
    return localStorage.getItem(this.tokenKey);
  }

  static getAuthHeaders(): Record<string, string> {
    const currentToken = this.getToken();
    return currentToken
      ? { 'Authorization': `Bearer ${currentToken}` }
      : {};
  }

  private static setAuth(authResponse: AuthResponse) {
    localStorage.setItem(this.tokenKey, authResponse.token);
    token.set(authResponse.token);
    user.set(authResponse.user);
  }
}

export default AuthService;