/** Small colored circle indicating status. DOM: `div.status-dot.{variant}`. */

export type StatusDotVariant = 'clean' | 'modified' | 'pending' | 'error';

export function createStatusDot(variant: StatusDotVariant): HTMLElement {
  const el = document.createElement('div');
  el.className = `status-dot ${variant}`;
  return el;
}
