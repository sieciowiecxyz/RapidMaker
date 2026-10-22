export function navigate(path: string) {
  const target = new URL(path, location.origin);
  if (target.pathname === '/' || target.pathname === '/wycena') {
    if (target.pathname === '/wycena') target.searchParams.set('restore', 'cart');
    location.assign(target.href);
    return;
  }
  history.pushState({}, '', path);
  window.dispatchEvent(new Event('rapidmaker:navigate'));
}
