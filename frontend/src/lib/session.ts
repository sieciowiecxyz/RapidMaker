import type { Session } from './types';

async function sessionRequest<T>(method = 'GET', body?: unknown): Promise<T> {
  const response = await fetch('/api/admin/session', {
    method,
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  const data = await response.json();
  if (!response.ok)
    throw new Error(data.message ?? 'Nie udało się sprawdzić sesji administratora.');
  return data as T;
}

export async function operatorSession(): Promise<Session | null> {
  const status = await sessionRequest<{ authenticated: boolean; session: Session | null }>();
  return status.authenticated ? status.session : null;
}
export async function loginOperator(token: string): Promise<void> {
  await sessionRequest('POST', { token });
}
export async function logoutOperator(): Promise<void> {
  await sessionRequest('DELETE');
}
