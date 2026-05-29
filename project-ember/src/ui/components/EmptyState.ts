/** Centered empty-state message. DOM: `div.empty-state`. */
export function createEmptyState(message: string): HTMLElement {
  const el = document.createElement('div');
  el.className = 'empty-state';
  el.textContent = message;
  return el;
}
