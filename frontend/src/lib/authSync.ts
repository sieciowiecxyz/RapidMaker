// Cross-document invalidation carries only a random signal, never session data.
const key = 'rapidmaker:auth-change';
let epoch = 0;
const listeners = new Set<() => void>();
function storedSignal(): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}
let seen = storedSignal();
const channel = typeof BroadcastChannel === 'undefined' ? null : new BroadcastChannel(key);
export const authEpoch = () => epoch;
export function announceAuthChange() {
  epoch++;
  seen = crypto.randomUUID();
  try {
    localStorage.setItem(key, seen);
  } catch {
    /* BroadcastChannel still works. */
  }
  channel?.postMessage(seen);
  // The initiating document keeps its login/MFA flow and processes its own result.
}
function invalidate() {
  epoch++;
  for (const listener of listeners) listener();
}
function changed(signal: unknown) {
  if (typeof signal !== 'string' || signal === seen) return;
  seen = signal;
  invalidate();
}
function check() {
  changed(storedSignal());
}
// The durable random marker catches changes missed while a document was suspended.
window.addEventListener('storage', (event) => {
  if (event.key === key) changed(event.newValue);
});
channel?.addEventListener('message', (event) => changed(event.data));
window.addEventListener('focus', check);
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible') check();
});
window.addEventListener('pageshow', (event) => {
  if (event.persisted) {
    seen = storedSignal();
    invalidate();
  } else check();
});
export function watchAuthChanges(invalidate: () => void): () => void {
  listeners.add(invalidate);
  check();
  return () => {
    listeners.delete(invalidate);
  };
}
