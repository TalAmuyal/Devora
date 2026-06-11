import { invoke } from '@tauri-apps/api/core';
import { createErrorNotification } from './ui/components/ErrorNotification';

const errors: string[] = [];

/** Record an error. Called by error-handling code throughout the app. */
export function recordError(message: string): void {
  errors.push(message);
}

/** Return all errors recorded since the last scrape, then clear the list. */
export function scrapeErrors(): string[] {
  return errors.splice(0);
}

/** Fire-and-forget write to the Rust log file.
 *  Never banners, never records.
 *  log_error failures are deliberately swallowed: reporting them would recurse back into the error path. */
export function logToFile(level: 'ERROR' | 'WARN', message: string): void {
  invoke('log_error', { level, message }).catch(() => {});
}

interface VisibleBanner {
  element: HTMLElement;
  count: number;
  countEl: HTMLSpanElement | null;
}

let stackEl: HTMLElement | null = null;
const visibleBanners = new Map<string, VisibleBanner>();

function ensureStack(): HTMLElement {
  if (!stackEl || !stackEl.isConnected) {
    stackEl = document.createElement('div');
    stackEl.className = 'error-banner-stack';
    document.body.appendChild(stackEl);
  }
  return stackEl;
}

function bumpCount(banner: VisibleBanner): void {
  banner.count += 1;
  if (!banner.countEl) {
    banner.countEl = document.createElement('span');
    banner.countEl.className = 'error-banner-count';
    const dismissBtn = banner.element.querySelector('.ws-error-notification-dismiss');
    banner.element.insertBefore(banner.countEl, dismissBtn);
  }
  banner.countEl.textContent = `×${banner.count}`;
}

function addBanner(message: string): void {
  const handle = createErrorNotification(message, () => {
    handle.element.remove();
    visibleBanners.delete(message);
  });
  visibleBanners.set(message, { element: handle.element, count: 1, countEl: null });
  ensureStack().appendChild(handle.element);
}

/** THE single way to surface an error: records it for test scraping, writes it to the Rust log file, and shows a dismissible banner in the global stack.
 *  Identical visible messages collapse into one banner with a ×N counter. */
export function showError(message: string): void {
  recordError(message);
  logToFile('ERROR', message);
  try {
    const existing = visibleBanners.get(message);
    if (existing && existing.element.isConnected) {
      bumpCount(existing);
    } else {
      visibleBanners.delete(message);
      addBanner(message);
    }
  } catch {
    // Error display must never throw: showError runs inside the global error handlers, where a throw would loop back into this function.
  }
}

/** Remove all banners and reset the dedupe state. Used by tests and the BDD harness. */
export function clearErrorBanners(): void {
  for (const banner of visibleBanners.values()) {
    banner.element.remove();
  }
  visibleBanners.clear();
}

/** Routes uncaught errors and unhandled rejections into showError, and mirrors console.error/console.warn into the Rust log file.
 * Called once from main.ts.
 */
export function installGlobalErrorHandlers(): void {
  window.addEventListener('error', (e) => {
    showError(`${e.message} at ${e.filename}:${e.lineno}:${e.colno}\n${e.error?.stack ?? ''}`);
  });

  window.addEventListener('unhandledrejection', (e) => {
    showError(`Unhandled rejection: ${e.reason}`);
  });

  const origConsoleError = console.error.bind(console);
  console.error = (...args: unknown[]) => {
    origConsoleError(...args);
    logToFile('ERROR', args.map(String).join(' '));
  };

  const origConsoleWarn = console.warn.bind(console);
  console.warn = (...args: unknown[]) => {
    origConsoleWarn(...args);
    logToFile('WARN', args.map(String).join(' '));
  };
}
