import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createDropdownMenu, DropdownItem } from '../DropdownMenu';
import { pageLayer } from '../../layers/presets';
import { getWindowLayerStack } from '../../layers/stack';
import { installModalStack, teardownModalStack, pressKey } from './modalTestHarness';
import type { LayerHandle } from '../../layers/types';

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

/** Push a `page` layer holding the dropdown, mirroring production where a dropdown lives inside a hub page. */
function hostDropdownInPage(
  element: HTMLElement,
  onKey?: (e: KeyboardEvent) => boolean,
): LayerHandle {
  const page = document.createElement('div');
  page.className = 'host-page';
  page.appendChild(element);
  return getWindowLayerStack().push(pageLayer({ name: 'host-page', element: page, onKey }));
}

function makeRect(rect: Partial<DOMRect>): DOMRect {
  return {
    top: 0,
    bottom: 0,
    left: 0,
    right: 0,
    width: 0,
    height: 0,
    x: 0,
    y: 0,
    toJSON: () => ({}),
    ...rect,
  } as DOMRect;
}

describe('createDropdownMenu', () => {
  beforeEach(() => installModalStack());
  afterEach(() => {
    teardownModalStack();
    vi.restoreAllMocks();
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

  it('pushes the open popup as a popup layer on top of the stack', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    openDropdown(handle.element);

    const top = getWindowLayerStack().top();
    expect(top?.name).toBe('dropdown-menu');
    expect(top?.element).toBe(handle.element.querySelector('.dropdown-popup'));
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
    expect(getWindowLayerStack().isEmpty()).toBe(true);
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
    expect(getWindowLayerStack().isEmpty()).toBe(true);
  });

  it('closes only the dropdown on Escape and does not consult the host page', () => {
    const pageOnKey = vi.fn(() => false);
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    const page = hostDropdownInPage(handle.element, pageOnKey);
    openDropdown(handle.element);

    const e = pressKey({ key: 'Escape', code: 'Escape' });

    expect(handle.element.querySelector('.dropdown-popup')).toBeNull();
    expect(getWindowLayerStack().top()).toBe(page);
    expect(pageOnKey).not.toHaveBeenCalled();
    expect(e.defaultPrevented).toBe(true);
  });

  it('closes the dropdown on q (vim-style dismissal)', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    hostDropdownInPage(handle.element);
    openDropdown(handle.element);

    pressKey({ key: 'q', code: 'KeyQ' });

    expect(handle.element.querySelector('.dropdown-popup')).toBeNull();
  });

  it('is removed together with its host page (no leaked outside-click listener)', () => {
    const addSpy = vi.spyOn(document, 'addEventListener');
    const removeSpy = vi.spyOn(document, 'removeEventListener');
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    const page = hostDropdownInPage(handle.element);
    openDropdown(handle.element);
    const outsideClick = capturedClickListener(addSpy);

    getWindowLayerStack().remove(page);

    expect(handle.element.querySelector('.dropdown-popup')).toBeNull();
    expect(handle.element.classList.contains('dropdown-open')).toBe(false);
    expect(removeSpy).toHaveBeenCalledWith('click', outsideClick, true);
    addSpy.mockRestore();
    removeSpy.mockRestore();
  });

  it('removes its outside-click listener on Escape (no leak)', () => {
    const addSpy = vi.spyOn(document, 'addEventListener');
    const removeSpy = vi.spyOn(document, 'removeEventListener');
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    hostDropdownInPage(handle.element);
    openDropdown(handle.element);
    const outsideClick = capturedClickListener(addSpy);

    pressKey({ key: 'Escape', code: 'Escape' });

    expect(removeSpy).toHaveBeenCalledWith('click', outsideClick, true);
    addSpy.mockRestore();
    removeSpy.mockRestore();
  });

  it('does not intercept keys while closed', () => {
    const spy = vi.fn();
    window.addEventListener('keydown', spy, true);
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    const e = pressKey({ key: 'Escape', code: 'Escape' });

    expect(spy).toHaveBeenCalledOnce();
    expect(e.defaultPrevented).toBe(false); // an empty stack consumes nothing
    window.removeEventListener('keydown', spy, true);
  });

  it('setItems replaces the rows of an open popup', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    openDropdown(handle.element);
    handle.setItems([{ kind: 'option', label: 'Only', onSelect: vi.fn() }]);

    const popup = handle.element.querySelector('.dropdown-popup')!;
    expect(popup.querySelectorAll('.dropdown-item')).toHaveLength(1);
    expect(popup.textContent).toContain('Only');
    expect(getWindowLayerStack().depth()).toBe(1);
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
    expect(getWindowLayerStack().isEmpty()).toBe(true);
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

  it('places the popup below the container, right-aligned, via fixed viewport coordinates', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);
    vi.spyOn(handle.element, 'getBoundingClientRect').mockReturnValue(
      makeRect({ top: 100, bottom: 130, left: 200, right: 320 }),
    );

    openDropdown(handle.element);

    const popup = handle.element.querySelector('.dropdown-popup') as HTMLElement;
    expect(popup.style.top).toBe('136px');
    expect(popup.style.right).toBe(`${window.innerWidth - 320}px`);
    expect(popup.style.bottom).toBe('');
  });

  it('flips upward when the popup would not fit below the container', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);
    const viewportHeight = window.innerHeight;
    // The popup doesn't exist until open, so the prototype mock supplies its 200px height
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(
      makeRect({ height: 200 }),
    );
    vi.spyOn(handle.element, 'getBoundingClientRect').mockReturnValue(
      makeRect({ top: viewportHeight - 68, bottom: viewportHeight - 38, left: 200, right: 320 }),
    );

    openDropdown(handle.element);

    const popup = handle.element.querySelector('.dropdown-popup') as HTMLElement;
    expect(popup.style.bottom).toBe('74px');
    expect(popup.style.top).toBe('');
  });

  it('closes when an ancestor of the dropdown scrolls', () => {
    const scroller = document.createElement('div');
    document.body.appendChild(scroller);
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    scroller.appendChild(handle.element);

    openDropdown(handle.element);
    scroller.dispatchEvent(new Event('scroll'));

    expect(handle.element.querySelector('.dropdown-popup')).toBeNull();
  });

  it('stays open when an unrelated element scrolls (e.g. a background terminal)', () => {
    const unrelated = document.createElement('div');
    document.body.appendChild(unrelated);
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    openDropdown(handle.element);
    unrelated.dispatchEvent(new Event('scroll'));

    expect(handle.element.querySelector('.dropdown-popup')).not.toBeNull();
  });

  it('closes on window resize', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    openDropdown(handle.element);
    window.dispatchEvent(new Event('resize'));

    expect(handle.element.querySelector('.dropdown-popup')).toBeNull();
  });

  // Overlay teardown removes the dropdown's subtree without calling close().
  it('cleans up via the scroll listener after the container is removed while open', () => {
    const handle = createDropdownMenu({ triggerLabel: 'Work', items: makeItems() });
    document.body.appendChild(handle.element);

    openDropdown(handle.element);
    handle.element.remove();
    document.body.dispatchEvent(new Event('scroll'));

    expect(handle.element.querySelector('.dropdown-popup')).toBeNull();
  });
});

/** The capture-phase document 'click' listener the open dropdown registered, recovered from a spy. */
function capturedClickListener(addSpy: ReturnType<typeof vi.spyOn>): EventListener {
  const call = addSpy.mock.calls.find(([type]) => type === 'click');
  if (!call) throw new Error('dropdown did not register a document click listener');
  return call[1] as EventListener;
}
