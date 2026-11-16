/**
 * Fixes a reflow bug in xterm.js 6.0.0's built bundles, run as npm's `postinstall`.
 *
 * Narrowing a terminal rewraps its lines; when the cursor is on the bottom row, `Buffer._reflowSmaller` scrolls the
 * viewport (`ybase++`) without checking that the buffer has room for the line it scrolls off. With `scrollback: 0`
 * (Ember's terminals) it never has: the cursor's row points past the buffer's end, so what is written next lands on
 * the wrong lines (over an older line, then wrapping around to the first, off screen) until the next resize.
 * The same function's other branch has the check; this applies it here too, so a line with no room is trimmed instead.
 *
 * Reported upstream as xtermjs/xterm.js#6331. xterm is pinned to the version the fix was written for; drop this script
 * when a release has the fix.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

export const PATCHED_XTERM_VERSION = '6.0.0';

// The bundles name the new row count differently (`t` in one, `e` in the other), hence the captures.
const UNCHECKED_SCROLL =
  /(0===this\.ybase|this\.ybase===0)\?this\.y<(\w+)-1\?\(this\.y\+\+,this\.lines\.pop\(\)\):\(this\.ybase\+\+,this\.ydisp\+\+\):(this\.ybase<Math\.min\(this\.lines\.maxLength,this\.lines\.length\+\w+\)-\2)&&/g;
const CHECKED_SCROLL =
  /(0===this\.ybase|this\.ybase===0)\?this\.y<(\w+)-1\?\(this\.y\+\+,this\.lines\.pop\(\)\):(this\.ybase<Math\.min\(this\.lines\.maxLength,this\.lines\.length\+\w+\)-\2)&&\(this\.ybase\+\+,this\.ydisp\+\+\):\3&&/g;

/** Returns the bundle with the fix applied, and whether it already had it; throws when the code isn't found exactly once. */
export function patchXtermBundle(source: string): { source: string; alreadyPatched: boolean } {
  const unchecked = [...source.matchAll(UNCHECKED_SCROLL)].length;
  const checked = [...source.matchAll(CHECKED_SCROLL)].length;
  if (unchecked === 0 && checked === 1) return { source, alreadyPatched: true };
  if (unchecked !== 1 || checked !== 0) {
    throw new Error(`expected the reflow's viewport adjustment once, found it ${unchecked} times unfixed and ${checked} fixed`);
  }
  return { source: source.replace(UNCHECKED_SCROLL, '$1?this.y<$2-1?(this.y++,this.lines.pop()):$3&&(this.ybase++,this.ydisp++):$3&&'), alreadyPatched: false };
}

function main(): void {
  const xterm = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'node_modules', '@xterm', 'xterm');
  const { version } = JSON.parse(fs.readFileSync(path.join(xterm, 'package.json'), 'utf8'));
  if (version !== PATCHED_XTERM_VERSION) {
    throw new Error(`scripts/patch-xterm.ts fixes xterm ${PATCHED_XTERM_VERSION}, but ${version} is installed: check whether it still has the bug, then update or drop the script`);
  }
  for (const bundle of ['lib/xterm.js', 'lib/xterm.mjs']) {
    const file = path.join(xterm, bundle);
    const { source, alreadyPatched } = patchXtermBundle(fs.readFileSync(file, 'utf8'));
    if (!alreadyPatched) fs.writeFileSync(file, source);
    console.log(`patch-xterm: ${bundle} ${alreadyPatched ? 'already fixed' : 'fixed'}`);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
