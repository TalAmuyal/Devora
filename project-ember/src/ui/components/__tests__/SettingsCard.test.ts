import { describe, it, expect } from 'vitest';
import { createSettingsCard } from '../SettingsCard';

describe('createSettingsCard', () => {
  it('renders a settings-card with a header strip showing the title', () => {
    const card = createSettingsCard('Repos');
    expect(card.className).toBe('settings-card');
    const header = card.querySelector('.settings-card-header');
    expect(header?.textContent).toBe('Repos');
  });

  it('puts the header first so appended content sits below it', () => {
    const card = createSettingsCard('Repos');
    const body = document.createElement('div');
    card.appendChild(body);
    expect(card.firstElementChild?.classList.contains('settings-card-header')).toBe(true);
    expect(card.lastElementChild).toBe(body);
  });
});
