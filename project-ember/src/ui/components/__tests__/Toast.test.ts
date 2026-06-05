import { describe, it, expect, afterEach, vi } from 'vitest';
import { createToast } from '../Toast';

describe('createToast', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.useRealTimers();
  });

  it('creates a div.toast appended to document.body', () => {
    const { element } = createToast('Refreshing…');
    expect(element.tagName).toBe('DIV');
    expect(element.classList.contains('toast')).toBe(true);
    expect(document.body.contains(element)).toBe(true);
  });

  it('sets the status role and aria-live for screen readers', () => {
    const { element } = createToast('Refreshing…');
    expect(element.getAttribute('role')).toBe('status');
    expect(element.getAttribute('aria-live')).toBe('polite');
  });

  it('shows the message', () => {
    const { element } = createToast('Refreshing…');
    expect(element.textContent).toBe('Refreshing…');
  });

  it('is visible immediately (not gated behind a fade-in class)', () => {
    const { element } = createToast('Refreshing…');
    expect(element.classList.contains('toast-hidden')).toBe(false);
  });

  it('dismiss() adds toast-hidden to fade out', () => {
    const { element, dismiss } = createToast('Refreshing…');
    void dismiss();
    expect(element.classList.contains('toast-hidden')).toBe(true);
  });

  it('dismiss() removes the element and resolves on transitionend', async () => {
    const { element, dismiss } = createToast('Refreshing…');
    const done = dismiss();
    element.dispatchEvent(new Event('transitionend'));
    await done;
    expect(document.body.contains(element)).toBe(false);
  });

  it('dismiss() removes the element via timeout fallback when transitionend never fires', () => {
    vi.useFakeTimers();
    const { element, dismiss } = createToast('Refreshing…');
    void dismiss();
    expect(document.body.contains(element)).toBe(true);
    vi.advanceTimersByTime(300);
    expect(document.body.contains(element)).toBe(false);
  });

  it('dismiss() is idempotent', () => {
    const { element, dismiss } = createToast('Refreshing…');
    void dismiss();
    element.dispatchEvent(new Event('transitionend'));
    expect(document.body.contains(element)).toBe(false);
    expect(() => void dismiss()).not.toThrow();
  });
});
