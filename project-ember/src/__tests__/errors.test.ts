import { describe, it, expect, vi, beforeEach, beforeAll } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
  showError,
  clearErrorBanners,
  scrapeErrors,
  logToFile,
  installGlobalErrorHandlers,
} from '../errors';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

const mockedInvoke = vi.mocked(invoke);

function getBanners(): HTMLElement[] {
  return [...document.querySelectorAll<HTMLElement>('.error-banner-stack .ws-error-notification')];
}

function flushMicrotasks(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

beforeEach(() => {
  clearErrorBanners();
  scrapeErrors();
  mockedInvoke.mockClear();
  mockedInvoke.mockImplementation(() => Promise.resolve(undefined));
});

describe('showError', () => {
  it('creates the banner stack on document.body and shows the message', () => {
    showError('something broke');
    const stack = document.querySelector('.error-banner-stack');
    expect(stack).not.toBeNull();
    expect(stack!.parentElement).toBe(document.body);
    const banners = getBanners();
    expect(banners).toHaveLength(1);
    expect(banners[0].textContent).toContain('something broke');
  });

  it('stacks two different messages as two banners, newest last', () => {
    showError('first');
    showError('second');
    const banners = getBanners();
    expect(banners).toHaveLength(2);
    expect(banners[0].textContent).toContain('first');
    expect(banners[1].textContent).toContain('second');
  });

  it('records the error and writes it to the Rust log file', () => {
    showError('boom');
    expect(scrapeErrors()).toEqual(['boom']);
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
    expect(mockedInvoke).toHaveBeenCalledWith('log_error', { level: 'ERROR', message: 'boom' });
  });

  it('dismiss removes only the dismissed banner', () => {
    showError('first');
    showError('second');
    const dismissFirst = getBanners()[0].querySelector<HTMLButtonElement>(
      '.ws-error-notification-dismiss',
    )!;
    dismissFirst.click();
    const banners = getBanners();
    expect(banners).toHaveLength(1);
    expect(banners[0].textContent).toContain('second');
  });

  it('collapses duplicate messages into one banner with a ×N counter', () => {
    showError('dup');
    showError('dup');
    let banners = getBanners();
    expect(banners).toHaveLength(1);
    expect(banners[0].querySelector('.error-banner-count')!.textContent).toBe('×2');

    showError('dup');
    expect(getBanners()[0].querySelector('.error-banner-count')!.textContent).toBe('×3');

    // Every occurrence is still recorded and logged, only the DOM is deduped.
    expect(scrapeErrors()).toEqual(['dup', 'dup', 'dup']);
    expect(mockedInvoke).toHaveBeenCalledTimes(3);

    // Dismiss resets the dedupe: a fresh banner appears without a counter.
    getBanners()[0].querySelector<HTMLButtonElement>('.ws-error-notification-dismiss')!.click();
    expect(getBanners()).toHaveLength(0);
    showError('dup');
    banners = getBanners();
    expect(banners).toHaveLength(1);
    expect(banners[0].querySelector('.error-banner-count')).toBeNull();
  });

  it('does not recurse or throw when the log_error invoke fails', async () => {
    mockedInvoke.mockImplementation(() => Promise.reject('log file gone'));
    expect(() => showError('boom')).not.toThrow();
    await flushMicrotasks();
    expect(getBanners()).toHaveLength(1);
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
    expect(scrapeErrors()).toEqual(['boom']);
  });

  it('re-creates the stack container if it was removed from the DOM', () => {
    showError('first');
    document.querySelector('.error-banner-stack')!.remove();
    showError('second');
    expect(document.querySelector('.error-banner-stack')).not.toBeNull();
    expect(getBanners()).toHaveLength(1);
    expect(getBanners()[0].textContent).toContain('second');

    // A message whose banner went away with the container gets a fresh banner.
    showError('first');
    expect(getBanners()).toHaveLength(2);
  });
});

describe('logToFile', () => {
  it('writes via log_error and never records or banners', () => {
    logToFile('WARN', 'just a warning');
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
    expect(mockedInvoke).toHaveBeenCalledWith('log_error', {
      level: 'WARN',
      message: 'just a warning',
    });
    expect(getBanners()).toHaveLength(0);
    expect(scrapeErrors()).toEqual([]);
  });

  it('swallows log_error failures', async () => {
    mockedInvoke.mockImplementation(() => Promise.reject('nope'));
    expect(() => logToFile('ERROR', 'x')).not.toThrow();
    await flushMicrotasks();
  });
});

describe('installGlobalErrorHandlers', () => {
  beforeAll(() => {
    installGlobalErrorHandlers();
  });

  it('surfaces window error events as banners', () => {
    window.dispatchEvent(
      new ErrorEvent('error', { message: 'kaboom', filename: 'file.ts', lineno: 3, colno: 7 }),
    );
    const banners = getBanners();
    expect(banners).toHaveLength(1);
    expect(banners[0].textContent).toContain('kaboom');
    expect(banners[0].textContent).toContain('file.ts:3:7');
    expect(scrapeErrors()).toHaveLength(1);
  });

  it('surfaces unhandled rejections as banners', () => {
    const event = new Event('unhandledrejection') as Event & { reason: unknown };
    event.reason = 'rejected-for-testing';
    window.dispatchEvent(event);
    const banners = getBanners();
    expect(banners).toHaveLength(1);
    expect(banners[0].textContent).toContain('rejected-for-testing');
    expect(scrapeErrors()).toHaveLength(1);
  });

  it('mirrors console.error to the log file without a banner', () => {
    console.error('diagnostic only');
    expect(mockedInvoke).toHaveBeenCalledWith('log_error', {
      level: 'ERROR',
      message: 'diagnostic only',
    });
    expect(getBanners()).toHaveLength(0);
    expect(scrapeErrors()).toEqual([]);
  });

  it('mirrors console.warn to the log file without a banner', () => {
    console.warn('warning only');
    expect(mockedInvoke).toHaveBeenCalledWith('log_error', {
      level: 'WARN',
      message: 'warning only',
    });
    expect(getBanners()).toHaveLength(0);
    expect(scrapeErrors()).toEqual([]);
  });
});
