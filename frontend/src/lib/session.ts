import type { EmailChallenge, Session } from './types';
import { ApiError, apiRequest, clearAuthToken } from './http';

type AuthResponse = {
  user: { id: string; email: string; role: 'client' | 'admin' };
  admin_verified: boolean;
};
export type AuthSession = Session & { adminVerified: boolean };
function session(data: AuthResponse): AuthSession {
  return {
    id: data.user.id,
    email: data.user.email,
    name: data.user.email,
    role: data.user.role,
    adminVerified: data.admin_verified,
  };
}
export async function getSession(): Promise<AuthSession | null> {
  try {
    return session(await apiRequest<AuthResponse>('/api/auth/me'));
  } catch (error) {
    if (error instanceof ApiError && error.status === 401) return null;
    throw error;
  }
}
export async function operatorSession(): Promise<AuthSession | null> {
  const current = await getSession();
  return current?.role === 'admin' && current.adminVerified ? current : null;
}
export async function requestLogin(email: string, _register = false): Promise<EmailChallenge> {
  const result = await apiRequest<{ challenge_id: string }>('/api/auth/start', 'POST', { email });
  return { id: result.challenge_id, email: email.trim(), isNew: false };
}
export async function confirmLogin(challengeId: string, code: string): Promise<AuthSession> {
  clearAuthToken();
  return session(
    await apiRequest<AuthResponse>('/api/auth/verify', 'POST', { challenge_id: challengeId, code }),
  );
}
export async function logout(): Promise<void> {
  await apiRequest('/api/auth/logout', 'POST', {});
  clearAuthToken();
}
function decode(value: string): ArrayBuffer {
  return Uint8Array.from(atob(value.replace(/-/g, '+').replace(/_/g, '/')), (char) =>
    char.charCodeAt(0),
  ).buffer;
}
function encode(value: ArrayBuffer): string {
  return btoa(String.fromCharCode(...new Uint8Array(value)))
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}
export async function loginPasskey(): Promise<AuthSession> {
  if (!window.isSecureContext || !window.PublicKeyCredential)
    throw new Error('Passkey wymaga HTTPS lub adresu http://localhost:18430.');
  type Options = Omit<PublicKeyCredentialRequestOptions, 'challenge' | 'allowCredentials'> & {
    challenge: string;
    allowCredentials?: (Omit<PublicKeyCredentialDescriptor, 'id'> & { id: string })[];
  };
  const start = await apiRequest<{ challenge_id: string; options: { publicKey: Options } }>(
    '/api/auth/passkey/login/start',
    'POST',
    {},
  );
  const options = start.options.publicKey;
  const credential = (await navigator.credentials.get({
    publicKey: {
      ...options,
      challenge: decode(options.challenge),
      allowCredentials: options.allowCredentials?.map((item) => ({ ...item, id: decode(item.id) })),
    },
  })) as PublicKeyCredential | null;
  if (!credential) throw new Error('Nie potwierdzono klucza dostępu.');
  const response = credential.response as AuthenticatorAssertionResponse;
  return session(
    await apiRequest<AuthResponse>('/api/auth/passkey/login/finish', 'POST', {
      challenge_id: start.challenge_id,
      credential: {
        id: credential.id,
        rawId: encode(credential.rawId),
        type: credential.type,
        response: {
          authenticatorData: encode(response.authenticatorData),
          clientDataJSON: encode(response.clientDataJSON),
          signature: encode(response.signature),
          userHandle: response.userHandle ? encode(response.userHandle) : null,
        },
        extensions: credential.getClientExtensionResults(),
      },
    }),
  );
}

// A new visitor has no guest cookie. All other claim failures must remain visible.
export async function claimGuest(): Promise<void> {
  try {
    await apiRequest('/api/auth/claim-guest', 'POST', {});
  } catch (error) {
    if (!(
      error instanceof ApiError &&
      error.status === 401 &&
      error.message === 'Brak sesji gościa.'
    ))
      throw error;
  }
}
