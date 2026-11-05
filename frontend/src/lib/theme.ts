export type Theme = 'light' | 'dark';

export function restoreTheme() {
  let theme: Theme = 'dark';
  try {
    if (localStorage.getItem('rapidmaker-concept-theme') === 'light') theme = 'light';
  } catch {
    /* Browser storage is optional. */
  }
  document.documentElement.dataset.theme = theme;
}

export function saveTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem('rapidmaker-concept-theme', theme);
  } catch {
    /* The current page still changes without persistent storage. */
  }
}
