export function navigate(path: string) {
  const target = new URL(path, location.origin);
  if (target.origin !== location.origin) throw new Error('Nieprawidłowy adres strony.');
  if (
    target.pathname === '/' ||
    target.pathname === '/wycena' ||
    target.pathname === '/admin.html' ||
    target.pathname.startsWith('/api/')
  ) {
    if (target.pathname === '/wycena') target.searchParams.set('restore', 'cart');
    location.assign(target.href);
    return;
  }
  history.pushState({}, '', path);
  window.dispatchEvent(new Event('rapidmaker:navigate'));
}
