/** Custom dropdown: a trigger button opening a popup of selectable options and action rows (with separators).
 * Closes on select, outside click, and Escape.
 * Returns a handle for imperative control.
 * DOM: `div.dropdown-menu`.
 */

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

export function createDropdownMenu(options: DropdownMenuOptions): DropdownMenuHandle {
  let items = options.items;
  let popup: HTMLElement | null = null;

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

  const onOutsideClick = (e: MouseEvent): void => {
    if (!container.contains(e.target as Node)) {
      close();
    }
  };

  // Capture phase so the dropdown swallows Escape before overlay-level dismissal handlers see it: Escape closes the popup, not the page.
  const onKeydown = (e: KeyboardEvent): void => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopImmediatePropagation();
      close();
    }
  };

  function close(): void {
    if (!popup) return;
    popup.remove();
    popup = null;
    container.classList.remove('dropdown-open');
    document.removeEventListener('click', onOutsideClick, true);
    document.removeEventListener('keydown', onKeydown, true);
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
    container.classList.add('dropdown-open');
    document.addEventListener('click', onOutsideClick, true);
    document.addEventListener('keydown', onKeydown, true);
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
