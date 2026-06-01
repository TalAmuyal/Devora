import { describe, it, expect, vi } from 'vitest';
import { createErrorNotification } from '../ErrorNotification';

describe('createErrorNotification', () => {
  it('returns an element with class ws-error-notification', () => {
    const { element } = createErrorNotification('oops', () => {});
    expect(element.tagName).toBe('DIV');
    expect(element.classList.contains('ws-error-notification')).toBe(true);
  });

  it('displays the error message', () => {
    const { element } = createErrorNotification('Something went wrong', () => {});
    const message = element.querySelector('.ws-error-notification-message');
    expect(message).not.toBeNull();
    expect(message!.textContent).toBe('Something went wrong');
  });

  it('has a dismiss button with text X', () => {
    const { element } = createErrorNotification('error', () => {});
    const btn = element.querySelector('.ws-error-notification-dismiss');
    expect(btn).not.toBeNull();
    expect(btn!.textContent).toBe('X');
  });

  it('calls onDismiss when dismiss button is clicked', () => {
    const onDismiss = vi.fn();
    const { element } = createErrorNotification('error', onDismiss);
    const btn = element.querySelector('.ws-error-notification-dismiss') as HTMLButtonElement;
    btn.click();
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it('calls onDismiss when dismiss() handle method is called', () => {
    const onDismiss = vi.fn();
    const handle = createErrorNotification('error', onDismiss);
    handle.dismiss();
    expect(onDismiss).toHaveBeenCalledOnce();
  });
});
