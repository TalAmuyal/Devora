/** Custom dropdown: a trigger button opening a popup of selectable options and action rows (with separators).
 * The open popup is a `popup` layer on the LayerStack (ADR-003), so Escape/q close only the dropdown (central dismissal) and it auto-closes when its host page is removed.
 * The popup is position:fixed so ancestor overflow clipping cannot hide it, and flips above the trigger when there is no room below.
 * It also closes on select, outside click, ancestor scroll, and window resize.
 * Returns a handle for imperative control.
 * DOM: `div.dropdown-menu`.
 */

import { getLayerStack } from '../layers/stack';
import type { LayerHandle } from '../layers/types';

export type DropdownItem =
  | {
      kind: 'option';
      label: string;
      /** Muted right-aligned text, e.g. a repo count. */
      detail?: string;
      /** Renders a checkmark; marks the currently active option. */
      checked?: boolean;
      onSelect: () => void;
    }
  | { kind: 'separator' }
  | {
      kind: 'action';
      label: string;
      icon?: string;
      onSelect: () => void;
    };

export interface DropdownMenuOptions {
  /** Text shown in the trigger. Omit when using `triggerContent` for an icon-only trigger. */
  triggerLabel?: string;
  /** Custom trigger content (e.g. a burger glyph) rendered in place of the text label. */
  triggerContent?: HTMLElement;
  /** Hide the ▾ chevron — e.g. for an icon-only trigger. */
  hideChevron?: boolean;
  /** Accessible name for an icon-only trigger; sets both `title` and `aria-label`. */
  triggerTitle?: string;
  items: DropdownItem[];
}

export interface DropdownMenuHandle {
  element: HTMLElement;
  close(): void;
  setItems(items: DropdownItem[]): void;
  setTriggerLabel(label: string): void;
}

const POPUP_GAP_PX = 6;

export function createDropdownMenu(options: DropdownMenuOptions): DropdownMenuHandle {
  let items = options.items;
  let popup: HTMLElement | null = null;
  let layerHandle: LayerHandle | null = null;

  const container = document.createElement('div');
  container.className = 'dropdown-menu';

  const trigger = document.createElement('button');
  trigger.className = 'dropdown-trigger';
  if (options.triggerTitle) {
    trigger.title = options.triggerTitle;
    trigger.setAttribute('aria-label', options.triggerTitle);
  }

  let triggerLabel: HTMLElement | null = null;
  if (options.triggerContent) {
    trigger.appendChild(options.triggerContent);
  } else {
    triggerLabel = document.createElement('span');
    triggerLabel.className = 'dropdown-trigger-label';
    triggerLabel.textContent = options.triggerLabel ?? '';
    trigger.appendChild(triggerLabel);
  }

  if (!options.hideChevron) {
    const chevron = document.createElement('span');
    chevron.className = 'dropdown-trigger-chevron';
    chevron.textContent = '▾';
    trigger.appendChild(chevron);
  }

  container.appendChild(trigger);

  // The container can be torn down without close() (e.g. overlay dismissal); handlers self-release.
  const closeIfDetached = (): boolean => {
    if (container.isConnected) return false;
    close();
    return true;
  };

  const onOutsideClick = (e: MouseEvent): void => {
    if (closeIfDetached()) return;
    if (!container.contains(e.target as Node)) {
      close();
    }
  };

  // Capture phase because scroll events do not bubble.
  // The fixed-position popup cannot track its anchor, so close when an ancestor scrolls - but only an ancestor: unrelated scrollers (e.g. a terminal viewport running behind an overlay) must not dismiss the menu
  const onAncestorScroll = (e: Event): void => {
    if (closeIfDetached()) return;
    if (e.target instanceof Node && e.target.contains(container)) {
      close();
    }
  };

  function close(): void {
    // The layer's onCleanup performs the teardown (below), so every close path funnels through one removal.
    if (layerHandle) {
      getLayerStack().remove(layerHandle);
    }
  }

  function open(): void {
    if (popup) return;
    popup = document.createElement('div');
    popup.className = 'dropdown-popup';

    for (const item of items) {
      if (item.kind === 'separator') {
        const sep = document.createElement('div');
        sep.className = 'dropdown-separator';
        popup.appendChild(sep);
        continue;
      }

      const row = document.createElement('button');
      row.className = `dropdown-item dropdown-item-${item.kind}`;

      if (item.kind === 'option') {
        const check = document.createElement('span');
        check.className = 'dropdown-item-check';
        check.textContent = item.checked ? '✓' : '';
        row.appendChild(check);
      } else if (item.icon) {
        const icon = document.createElement('span');
        icon.className = 'dropdown-item-icon';
        icon.textContent = item.icon;
        row.appendChild(icon);
      }

      const label = document.createElement('span');
      label.className = 'dropdown-item-label';
      label.textContent = item.label;
      row.appendChild(label);

      if (item.kind === 'option' && item.detail) {
        const detail = document.createElement('span');
        detail.className = 'dropdown-item-detail';
        detail.textContent = item.detail;
        row.appendChild(detail);
      }

      row.addEventListener('click', () => {
        close();
        item.onSelect();
      });
      popup.appendChild(row);
    }

    container.appendChild(popup);

    const rect = container.getBoundingClientRect();
    const popupHeight = popup.getBoundingClientRect().height;
    const overflowsBelow = rect.bottom + POPUP_GAP_PX + popupHeight > window.innerHeight;
    const moreRoomAbove = rect.top > window.innerHeight - rect.bottom;
    if (overflowsBelow && moreRoomAbove) {
      popup.style.bottom = `${window.innerHeight - rect.top + POPUP_GAP_PX}px`;
    } else {
      popup.style.top = `${rect.bottom + POPUP_GAP_PX}px`;
    }
    popup.style.right = `${window.innerWidth - rect.right}px`;

    container.classList.add('dropdown-open');
    document.addEventListener('click', onOutsideClick, true);
    window.addEventListener('scroll', onAncestorScroll, true);
    window.addEventListener('resize', close);

    // A `popup` layer stays where it is in the DOM and is transparent to keys it does not handle; Escape/q hit the stack's default dismissal, which pops it.
    layerHandle = getLayerStack().push({
      name: 'dropdown-menu',
      kind: 'popup',
      element: popup,
      onCleanup: () => {
        popup = null;
        layerHandle = null;
        container.classList.remove('dropdown-open');
        document.removeEventListener('click', onOutsideClick, true);
        window.removeEventListener('scroll', onAncestorScroll, true);
        window.removeEventListener('resize', close);
      },
    });
  }

  trigger.addEventListener('click', () => {
    if (popup) {
      close();
    } else {
      open();
    }
  });

  return {
    element: container,
    close,
    setItems(next: DropdownItem[]) {
      items = next;
      if (popup) {
        // Rebuild the open popup so the change is visible immediately.
        close();
        open();
      }
    },
    setTriggerLabel(label: string) {
      if (triggerLabel) {
        triggerLabel.textContent = label;
      }
    },
  };
}
