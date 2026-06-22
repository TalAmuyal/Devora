import { describe, it, expect, vi, afterEach } from 'vitest';
import { createDropdownMenu, DropdownItem } from '../DropdownMenu';

function makeItems(overrides?: { onWork?: () => void; onNew?: () => void }): DropdownItem[] {
  return [
    {
      kind: 'option',
      label: 'Work',
      detail: '12 repos',
      checked: true,
      onSelect: overrides?.onWork ?? vi.fn(),
    },
    { kind: 'option', label: 'Personal', onSelect: vi.fn() },
    { kind: 'separator' },
    { kind: 'action', label: 'New Profile…', icon: '＋', onSelect: overrides?.onNew ?? vi.fn() },
  ];
}

function openDropdown(element: HTMLElement): void {
  (element.querySelector('.dropdown-trigger') as HTMLButtonElement).click();
}

describe('createDropdownMenu', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders the trigger label and no popup initially', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    expect(handle.element.querySelector('.dropdown-trigger-label')?.textContent).toBe('Work');
    expect(handle.element.querySelector('.dropdown-popup')).toBeNull();
  });

  it('opens the popup on trigger click and renders all item kinds', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    openDropdown(handle.element);

    const popup = handle.element.querySelector('.dropdown-popup')!;
    expect(popup).not.toBeNull();
    expect(popup.querySelectorAll('.dropdown-item-option')).toHaveLength(2);
    expect(popup.querySelectorAll('.dropdown-separator')).toHaveLength(1);
    expect(popup.querySelectorAll('.dropdown-item-action')).toHaveLength(1);
  });

  it('renders checkmark and detail on the checked option', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    openDropdown(handle.element);

    const options = handle.element.querySelectorAll('.dropdown-item-option');
    expect(options[0].querySelector('.dropdown-item-check')?.textContent).toBe('✓');
    expect(options[0].querySelector('.dropdown-item-detail')?.textContent).toBe('12 repos');
    expect(options[1].querySelector('.dropdown-item-check')?.textContent).toBe('');
  });

  it('renders the icon on action items', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    openDropdown(handle.element);

    const action = handle.element.querySelector('.dropdown-item-action')!;
    expect(action.querySelector('.dropdown-item-icon')?.textContent).toBe('＋');
    expect(action.textContent).toContain('New Profile…');
  });

  it('fires onSelect and closes when an item is clicked', () => {
    const onNew = vi.fn();
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems({ onNew }) });
    document.body.appendChild(handle.element);

    openDropdown(handle.element);
    (handle.element.querySelector('.dropdown-item-action') as HTMLButtonElement).click();

    expect(onNew).toHaveBeenCalledOnce();
    expect(handle.element.querySelector('.dropdown-popup')).toBeNull();
  });

  it('closes on outside click without firing any onSelect', () => {
    const onWork = vi.fn();
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems({ onWork }) });
    document.body.appendChild(handle.element);
    const outside = document.createElement('div');
    document.body.appendChild(outside);

    openDropdown(handle.element);
    outside.dispatchEvent(new MouseEvent('click', { bubbles: true }));

    expect(handle.element.querySelector('.dropdown-popup')).toBeNull();
    expect(onWork).not.toHaveBeenCalled();
  });

  it('toggles closed when the trigger is clicked again', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    openDropdown(handle.element);
    openDropdown(handle.element);

    expect(handle.element.querySelector('.dropdown-popup')).toBeNull();
  });

  it('closes on Escape without letting the event propagate', () => {
    const windowHandler = vi.fn();
    window.addEventListener('keydown', windowHandler);
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    openDropdown(handle.element);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

    expect(handle.element.querySelector('.dropdown-popup')).toBeNull();
    expect(windowHandler).not.toHaveBeenCalled();
    window.removeEventListener('keydown', windowHandler);
  });

  it('does not intercept Escape while closed', () => {
    const windowHandler = vi.fn();
    window.addEventListener('keydown', windowHandler);
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

    expect(windowHandler).toHaveBeenCalledOnce();
    window.removeEventListener('keydown', windowHandler);
  });

  it('setItems replaces the rows of an open popup', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    openDropdown(handle.element);
    handle.setItems([{ kind: 'option', label: 'Only', onSelect: vi.fn() }]);

    const popup = handle.element.querySelector('.dropdown-popup')!;
    expect(popup.querySelectorAll('.dropdown-item')).toHaveLength(1);
    expect(popup.textContent).toContain('Only');
  });

  it('setTriggerLabel updates the trigger text', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    handle.setTriggerLabel('Personal');

    expect(handle.element.querySelector('.dropdown-trigger-label')?.textContent).toBe('Personal');
  });

  it('close() is a no-op when already closed', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    expect(() => handle.close()).not.toThrow();
  });

  it('renders custom trigger content instead of a label span', () => {
    const glyph = document.createElement('span');
    glyph.className = 'my-glyph';
    const handle = createDropdownMenu({ triggerContent: glyph, items: makeItems() });
    document.body.appendChild(handle.element);

    expect(handle.element.querySelector('.dropdown-trigger-label')).toBeNull();
    expect(handle.element.querySelector('.my-glyph')).not.toBeNull();
  });

  it('omits the chevron when hideChevron is set', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', hideChevron: true, items: makeItems() });
    document.body.appendChild(handle.element);

    expect(handle.element.querySelector('.dropdown-trigger-chevron')).toBeNull();
  });

  it('sets the trigger title and aria-label from triggerTitle', () => {
    const glyph = document.createElement('span');
    const handle = createDropdownMenu({ triggerContent: glyph, triggerTitle: 'Menu', items: makeItems() });
    document.body.appendChild(handle.element);

    const trigger = handle.element.querySelector('.dropdown-trigger')!;
    expect(trigger.getAttribute('title')).toBe('Menu');
    expect(trigger.getAttribute('aria-label')).toBe('Menu');
  });

  it('setTriggerLabel is a no-op when there is no label span', () => {
    const glyph = document.createElement('span');
    const handle = createDropdownMenu({ triggerContent: glyph, items: makeItems() });
    document.body.appendChild(handle.element);

    expect(() => handle.setTriggerLabel('Ignored')).not.toThrow();
    expect(handle.element.querySelector('.dropdown-trigger-label')).toBeNull();
  });
});
