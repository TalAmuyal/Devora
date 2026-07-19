/**
 * Whether keyboard focus is currently on an editable element (text inputs, selects, contentEditable regions).
 * Key handlers use this to leave typing alone — e.g. `q` must insert a "q" instead of closing an overlay.
 */
export function isEditableElementFocused(): boolean {
  const active = document.activeElement;
  if (!(active instanceof HTMLElement)) {
    return false;
  }
  return (
    active.tagName === 'INPUT' ||
    active.tagName === 'TEXTAREA' ||
    active.tagName === 'SELECT' ||
    active.isContentEditable
  );
}

/**
 * Make a text field unfocus on Escape — the Ember convention that a first Escape blurs the input and a second Escape then dismisses the page (ADR-003 routes Escape through to a focused editable inside a page, so the field itself must blur).
 * Matches the inline handling in SearchInput and ProfileForm.
 */
export function blurOnEscape(input: HTMLElement): void {
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopImmediatePropagation();
      input.blur();
    }
  });
}
