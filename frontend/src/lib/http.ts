// Session secrets stay in HttpOnly cookies; only the CSRF token lives in memory.
let csrfToken: string | null = null;
let csrfLoading: Promise<void> | null = null;
let generation = 0;

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
    public code?: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}
export function clearAuthToken() {
  csrfToken = null;
  csrfLoading = null;
  generation++;
}
function rememberToken(data: unknown) {
  if (
    data &&
    typeof data === 'object' &&
    'csrf_token' in data &&
    typeof data.csrf_token === 'string'
  )
    csrfToken = data.csrf_token;
}
async function loadCsrf() {
  if (!csrfLoading) {
    const currentGeneration = generation;
    const loading = (async () => {
      const response = await fetch('/api/auth/me', {
        credentials: 'same-origin',
        redirect: 'error',
        cache: 'no-store',
      });
      if (response.status === 401) return;
      if (!response.ok)
        throw new ApiError(response.status, 'Nie udało się sprawdzić sesji. Spróbuj ponownie.');
      const data: unknown = await response.json();
      if (currentGeneration === generation) rememberToken(data);
    })();
    csrfLoading = loading;
    void loading
      .finally(() => {
        if (csrfLoading === loading) csrfLoading = null;
      })
      .catch(() => {});
  }
  await csrfLoading;
}
export async function apiFetch(path: string, init: RequestInit = {}): Promise<Response> {
  const url = new URL(path, location.origin);
  if (
    !path.startsWith('/api/') ||
    url.origin !== location.origin ||
    !url.pathname.startsWith('/api/') ||
    url.hash
  )
    throw new ApiError(0, 'Nieprawidłowy adres API.');
  const method = (init.method ?? 'GET').toUpperCase();
  const headers = new Headers(init.headers);
  headers.delete('X-CSRF-Token');
  if (
    !['GET', 'HEAD', 'OPTIONS'].includes(method) &&
    ![
      '/api/auth/start',
      '/api/auth/verify',
      '/api/auth/password/login',
      '/api/auth/passkey/login/start',
    ].includes(url.pathname)
  ) {
    if (!csrfToken) await loadCsrf();
    if (csrfToken) headers.set('X-CSRF-Token', csrfToken);
  }
  const response = await fetch(url, {
    ...init,
    method,
    headers,
    credentials: 'same-origin',
    redirect: 'error',
    cache: 'no-store',
  });
  if (response.status === 401) {
    clearAuthToken();
    if (!url.pathname.startsWith('/api/auth/'))
      window.dispatchEvent(new Event('rapidmaker:session-expired'));
  }
  return response;
}
export async function apiRequest<T>(path: string, method = 'GET', body?: unknown): Promise<T> {
  const response = await apiFetch(path, {
    method,
    ...(body === undefined
      ? {}
      : { headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) }),
  });
  const data = response.status === 204 ? undefined : await response.json().catch(() => undefined);
  if (!response.ok)
    throw new ApiError(
      response.status,
      typeof data?.message === 'string'
        ? data.message
        : 'Nie udało się wykonać operacji. Spróbuj ponownie.',
      typeof data?.code === 'string' ? data.code : undefined,
    );
  rememberToken(data);
  return data as T;
}
