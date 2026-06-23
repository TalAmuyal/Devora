/**
 * A titled card surface: a bordered, rounded container with a header strip, matching the Claude Models & Effort card's chrome (shared CSS, see `.settings-card` in components.css).
 * The caller appends the body content (a table, rows, or a message) after the header.
 * DOM: `div.settings-card > div.settings-card-header + <caller content>`.
 */
export function createSettingsCard(title: string): HTMLElement {
  const card = document.createElement('div');
  card.className = 'settings-card';

  const header = document.createElement('div');
  header.className = 'settings-card-header';
  header.textContent = title;
  card.appendChild(header);

  return card;
}
