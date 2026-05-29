/** Search/filter text input with icon, focus management, and Escape handling. Returns a handle for imperative control. DOM: `div.search-input`. */

export interface SearchInputOptions {
  placeholder: string;
  value: string;
  icon: string;
  onInput: (value: string) => void;
  onEscape: () => void;
}

export interface SearchInputHandle {
  element: HTMLElement;
  focus(): void;
  getValue(): string;
}

export function createSearchInput(options: SearchInputOptions): SearchInputHandle {
  const container = document.createElement('div');
  container.className = 'search-input';

  const iconSpan = document.createElement('span');
  iconSpan.className = 'search-input-icon';
  iconSpan.textContent = options.icon;
  container.appendChild(iconSpan);

  const input = document.createElement('input');
  input.type = 'text';
  input.className = 'search-input-field';
  input.placeholder = options.placeholder;
  input.value = options.value;

  input.addEventListener('input', () => {
    options.onInput(input.value);
  });

  input.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopImmediatePropagation();
      input.blur();
      options.onEscape();
    }
  });

  container.appendChild(input);

  return {
    element: container,
    focus() {
      input.focus();
      const len = input.value.length;
      input.setSelectionRange(len, len);
    },
    getValue() {
      return input.value;
    },
  };
}
