import { apiRequest } from './http';

export type Passkey = { id: string; name: string; created_at: number };
export function listPasskeys() {
  return apiRequest<{ passkeys: Passkey[] }>('/api/auth/passkeys');
}
export function removePasskey(id: string) {
  return apiRequest('/api/auth/passkeys/' + encodeURIComponent(id), 'DELETE');
}
function bytes(value: string): ArrayBuffer {
  return Uint8Array.from(atob(value.replace(/-/g, '+').replace(/_/g, '/')), (c) => c.charCodeAt(0))
    .buffer;
}
function base64(value: ArrayBuffer): string {
  return btoa(String.fromCharCode(...new Uint8Array(value)))
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}
export async function registerPasskey(name: string, bootstrap?: string): Promise<void> {
  if (!window.isSecureContext || !window.PublicKeyCredential)
    throw new Error('Klucz dostępu wymaga HTTPS lub localhost.');
  type Options = Omit<
    PublicKeyCredentialCreationOptions,
    'challenge' | 'user' | 'excludeCredentials'
  > & {
    challenge: string;
    user: Omit<PublicKeyCredentialUserEntity, 'id'> & { id: string };
    excludeCredentials?: (Omit<PublicKeyCredentialDescriptor, 'id'> & { id: string })[];
  };
  const start = await apiRequest<{ challenge_id: string; options: { publicKey: Options } }>(
    '/api/auth/passkey/register/start',
    'POST',
    bootstrap ? { bootstrap } : {},
  );
  const options = start.options.publicKey;
  const credential = (await navigator.credentials.create({
    publicKey: {
      ...options,
      challenge: bytes(options.challenge),
      user: { ...options.user, id: bytes(options.user.id) },
      excludeCredentials: options.excludeCredentials?.map((item) => ({
        ...item,
        id: bytes(item.id),
      })),
    },
  })) as PublicKeyCredential | null;
  if (!credential) throw new Error('Nie zarejestrowano klucza.');
  const response = credential.response as AuthenticatorAttestationResponse;
  await apiRequest('/api/auth/passkey/register/finish', 'POST', {
    challenge_id: start.challenge_id,
    name: name.trim(),
    credential: {
      id: credential.id,
      rawId: base64(credential.rawId),
      type: credential.type,
      response: {
        attestationObject: base64(response.attestationObject),
        clientDataJSON: base64(response.clientDataJSON),
        transports: response.getTransports?.() ?? [],
      },
      clientExtensionResults: credential.getClientExtensionResults(),
    },
  });
}
