import { describe, it, expect } from 'vitest';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * Preview panes render user-controlled Markdown via innerHTML (and HTML in a sandboxed iframe).
 * The Content-Security-Policy is the defense that keeps that safe: it must block inline scripts and lock outbound connections to the IPC bridge.
 * This test fails if the CSP is weakened in a way that would let previewed content execute scripts or phone home.
 */

const CONF = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../src-tauri/tauri.conf.json',
);

function csp(): string {
  const conf = JSON.parse(fs.readFileSync(CONF, 'utf8'));
  return conf.app.security.csp as string;
}

describe('Content-Security-Policy', () => {
  it('defaults to self (blocks external resources and inline scripts)', () => {
    expect(csp()).toContain("default-src 'self'");
  });

  it('never allows inline or remote scripts', () => {
    const value = csp();
    expect(value).not.toContain("script-src 'unsafe-inline'");
    expect(value).not.toContain("script-src 'unsafe-eval'");
    expect(value).not.toMatch(/script-src[^;]*\*/);
  });

  it('locks outbound connections to the IPC bridge', () => {
    expect(csp()).toContain('connect-src ipc: http://ipc.localhost');
  });

  it('only frames self and localhost (no wildcard frame source)', () => {
    const value = csp();
    expect(value).toContain('frame-src');
    expect(value).not.toMatch(/frame-src[^;]*\s\*(\s|;|$)/);
  });
});
