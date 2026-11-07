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

// Revalidate protected views when returning from another tab/app/device session.
// Keep this separate from auth-change subscribers: login/MFA must keep their flow.
export function watchProtectedReturn(revalidate: () => void): () => void {
  let away = document.visibilityState === 'hidden' || !document.hasFocus();
  let blurTimer: ReturnType<typeof setTimeout> | undefined;
  function leave() {
    away = true;
  }
  function blur() {
    if (!document.hasFocus()) leave();
    // Focusing an iframe blurs window while the document still owns focus.
    // Let the browser settle focus before treating this as leaving the app.
    clearTimeout(blurTimer);
    blurTimer = setTimeout(() => {
      if (!document.hasFocus()) leave();
    }, 0);
  }
  function returned() {
    if (!away || document.visibilityState === 'hidden') return;
    away = false;
    revalidate();
  }
  function focus(event: FocusEvent) {
    if (event.target === window) returned();
  }
  function visibility() {
    if (document.visibilityState === 'hidden') leave();
    else returned();
  }
  function pageshow(event: PageTransitionEvent) {
    if (event.persisted) {
      away = true;
      returned();
    }
  }
  window.addEventListener('blur', blur);
  window.addEventListener('focus', focus);
  document.addEventListener('visibilitychange', visibility);
  window.addEventListener('pageshow', pageshow);
  return () => {
    clearTimeout(blurTimer);
    window.removeEventListener('blur', blur);
    window.removeEventListener('focus', focus);
    document.removeEventListener('visibilitychange', visibility);
    window.removeEventListener('pageshow', pageshow);
  };
}
