import { describe, it, expect } from 'vitest';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * The invoke wrapper (src/invoke.ts) and the error module (src/errors.ts) are the only sanctioned users of the raw Tauri core API.
 * Everything else must go through them, so that a failed invoke is surfaced to the user by default.
 */

const SRC_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const ALLOWED_FILES = ['errors.ts', 'invoke.ts'].map((name) => path.join(SRC_DIR, name));

function walkTsFiles(dir: string): string[] {
  const files: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === '__tests__') continue; // tests reference the literal for vi.mock
      files.push(...walkTsFiles(fullPath));
    } else if (entry.name.endsWith('.ts')) {
      files.push(fullPath);
    }
  }
  return files;
}

describe('tauri core import boundary', () => {
  it('the sanctioned files exist (a rename cannot make this test vacuous)', () => {
    for (const file of ALLOWED_FILES) {
      expect(fs.existsSync(file), `${file} should exist`).toBe(true);
    }
  });

  it('only errors.ts and invoke.ts import @tauri-apps/api/core', () => {
    const offenders = walkTsFiles(SRC_DIR).filter((file) =>
      fs.readFileSync(file, 'utf8').includes('@tauri-apps/api/core'),
    );
    expect(offenders.sort()).toEqual([...ALLOWED_FILES].sort());
  });
});
