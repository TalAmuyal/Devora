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
