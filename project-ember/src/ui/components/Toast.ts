/** Transient status toast pinned to the bottom-centre of the window. DOM: `div.toast` on `document.body`. */

export interface ToastHandle {
  element: HTMLElement;
  /** Fade the toast out and remove it; the promise resolves once it's removed. Idempotent. */
  dismiss: () => Promise<void>;
}

/** Fade-out grace period matching the CSS opacity transition, used as a
 * fallback in case `transitionend` does not fire (e.g. in WKWebView). */
const TOAST_FADE_OUT_MS = 300;

/**
 * Create a transient toast and append it to `document.body`. It fades in via a
 * CSS keyframe animation on insertion and fades out (opacity transition) when
 * dismissed.
 *
 * Living on `document.body` (rather than a page container) keeps the toast
 * alive across re-renders that clear other containers.
 */
export function createToast(message: string): ToastHandle {
  const el = document.createElement('div');
  el.className = 'toast';
  el.setAttribute('role', 'status');
  el.setAttribute('aria-live', 'polite');
  el.textContent = message;
  document.body.appendChild(el);

  let dismissed = false;

  return {
    element: el,
    dismiss: () =>
      new Promise<void>((resolve) => {
        if (dismissed) {
          resolve();
          return;
        }
        dismissed = true;

        let finished = false;
        const finish = (): void => {
          if (finished) return;
          finished = true;
          el.remove();
          resolve();
        };

        el.classList.add('toast-hidden');
        el.addEventListener('transitionend', finish, { once: true });
        // Fallback in case the opacity transition (and thus `transitionend`)
        // does not fire in the host webview.
        setTimeout(finish, TOAST_FADE_OUT_MS);
      }),
  };
}
