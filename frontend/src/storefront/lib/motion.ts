// Adapted from Transitions.dev, 34-text-swap-soft.
// https://github.com/Jakubantalik/transitions.dev/blob/main/skills/transitions-dev/34-text-swap-soft.md
export function softValue(element: HTMLElement, initial: string) {
  let timer: ReturnType<typeof setTimeout>;
  element.textContent = initial;
  element.dataset.text = initial;
  const ms = (name: string) => parseFloat(getComputedStyle(document.documentElement).getPropertyValue(name)) || 0;
  return {
    update(next: string) {
      const previous = element.dataset.text ?? element.textContent ?? '';
      if (previous === next) return;
      element.dataset.text = next;
      clearTimeout(timer);
      if (matchMedia('(prefers-reduced-motion: reduce)').matches || document.documentElement.dataset.input === 'keyboard') {
        element.classList.remove('is-swapping');
        element.style.removeProperty('width');
        element.textContent = next;
        return;
      }
      const out = document.createElement('span');
      out.className = 't-text-soft-old'; out.setAttribute('aria-hidden', 'true'); out.textContent = previous;
      const incoming = document.createElement('span');
      incoming.className = 't-text-soft-new'; incoming.textContent = next;
      const oldWidth = element.getBoundingClientRect().width;
      element.classList.remove('is-swapping');
      element.replaceChildren(out, incoming);
      const newWidth = incoming.getBoundingClientRect().width;
      element.style.width = `${oldWidth}px`;
      void element.offsetWidth;
      element.classList.add('is-swapping');
      element.style.width = `${newWidth}px`;
      timer = setTimeout(() => {
        element.classList.remove('is-swapping'); element.style.removeProperty('width'); element.textContent = next;
      }, Math.max(ms('--text-soft-dur'), ms('--text-soft-resize-dur')) + 40);
    },
    destroy() { clearTimeout(timer); }
  };
}
