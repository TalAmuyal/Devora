/** Colored status badge/pill. DOM: `span.badge.badge-{variant}`. */

export type BadgeVariant = 'clean' | 'modified' | 'untracked' | 'pending' | 'error' | 'inactive';

export function createBadge(text: string, variant: BadgeVariant): HTMLElement {
  const el = document.createElement('span');
  el.className = `badge badge-${variant}`;
  el.textContent = text;
  return el;
}
