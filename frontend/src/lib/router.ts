export function navigate(path: string) {
  history.pushState({}, '', path);
  window.dispatchEvent(new Event('rapidmaker:navigate'));
}
