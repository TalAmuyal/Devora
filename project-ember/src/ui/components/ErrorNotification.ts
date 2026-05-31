/** Persistent error banner with dismiss button. DOM: `div.ws-error-notification`. */

export interface ErrorNotificationHandle {
  element: HTMLElement;
  dismiss: () => void;
}

export function createErrorNotification(
  message: string,
  onDismiss: () => void,
): ErrorNotificationHandle {
  const el = document.createElement('div');
  el.className = 'ws-error-notification';

  const text = document.createElement('span');
  text.className = 'ws-error-notification-message';
  text.textContent = message;
  el.appendChild(text);

  const dismissBtn = document.createElement('button');
  dismissBtn.className = 'ws-error-notification-dismiss';
  dismissBtn.textContent = 'X';
  dismissBtn.addEventListener('click', () => {
    onDismiss();
  });
  el.appendChild(dismissBtn);

  return {
    element: el,
    dismiss: () => onDismiss(),
  };
}
