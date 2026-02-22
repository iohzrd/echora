import {
  startRegistration,
  startAuthentication,
} from "@simplewebauthn/browser";
import AuthService, { type AuthResponse } from "./auth";
import { getApiBase } from "./config";
import { appFetch } from "./serverManager";

export interface PasskeyInfo {
  id: string;
  name: string;
  created_at: string;
  last_used_at: string | null;
}

export class PasskeyService {
  static async registerPasskey(name?: string): Promise<PasskeyInfo> {
    const apiBase = getApiBase();

    const optionsResponse = await appFetch(
      `${apiBase}/auth/passkey/register/start`,
      {
        method: "POST",
        headers: {
          ...AuthService.getAuthHeaders(),
          "Content-Type": "application/json",
        },
        body: JSON.stringify({}),
      },
    );

    if (!optionsResponse.ok) {
      const err = await optionsResponse.json();
      throw new Error(err.error || "Failed to start passkey registration");
    }

    const responseJSON = await optionsResponse.json();
    const optionsJSON = responseJSON.publicKey;

    const attResp = await startRegistration({ optionsJSON });

    const verifyResponse = await appFetch(
      `${apiBase}/auth/passkey/register/finish`,
      {
        method: "POST",
        headers: {
          ...AuthService.getAuthHeaders(),
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          credential: attResp,
          name: name || undefined,
        }),
      },
    );

    if (!verifyResponse.ok) {
      const err = await verifyResponse.json();
      throw new Error(err.error || "Failed to complete passkey registration");
    }

    return verifyResponse.json();
  }

  static async loginWithPasskey(username?: string): Promise<AuthResponse> {
    const apiBase = getApiBase();

    const optionsResponse = await appFetch(
      `${apiBase}/auth/passkey/login/start`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username: username || undefined }),
      },
    );

    if (!optionsResponse.ok) {
      const err = await optionsResponse.json();
      throw new Error(err.error || "Failed to start passkey authentication");
    }

    const responseData = await optionsResponse.json();
    const { challenge_key, publicKey: optionsJSON } = responseData;

    const assertionResp = await startAuthentication({ optionsJSON });

    const verifyResponse = await appFetch(
      `${apiBase}/auth/passkey/login/finish`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          credential: assertionResp,
          challenge_key,
        }),
      },
    );

    if (!verifyResponse.ok) {
      const err = await verifyResponse.json();
      throw new Error(err.error || "Passkey authentication failed");
    }

    return verifyResponse.json();
  }

  static async listPasskeys(): Promise<PasskeyInfo[]> {
    const apiBase = getApiBase();
    const response = await appFetch(`${apiBase}/auth/passkeys`, {
      headers: AuthService.getAuthHeaders(),
    });

    if (!response.ok) {
      const err = await response.json();
      throw new Error(err.error || "Failed to fetch passkeys");
    }

    return response.json();
  }

  static async deletePasskey(passkeyId: string): Promise<void> {
    const apiBase = getApiBase();
    const response = await appFetch(`${apiBase}/auth/passkeys/${passkeyId}`, {
      method: "DELETE",
      headers: AuthService.getAuthHeaders(),
    });

    if (!response.ok) {
      const err = await response.json();
      throw new Error(err.error || "Failed to delete passkey");
    }
  }

  static isSupported(): boolean {
    return (
      typeof window !== "undefined" &&
      typeof window.PublicKeyCredential !== "undefined"
    );
  }
}
