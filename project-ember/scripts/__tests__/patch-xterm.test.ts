import { describe, it, expect } from 'vitest';
import { patchXtermBundle } from '../patch-xterm';

// The reflow's viewport adjustment as each of xterm 6.0.0's bundles has it (lib/xterm.js, lib/xterm.mjs).
const UNFIXED = {
  'xterm.js':
    'for(;b-- >0;)0===this.ybase?this.y<t-1?(this.y++,this.lines.pop()):(this.ybase++,this.ydisp++):this.ybase<Math.min(this.lines.maxLength,this.lines.length+n)-t&&(this.ybase===this.ydisp&&this.ydisp++,this.ybase++);',
  'xterm.mjs':
    'for(;O-- >0;)this.ybase===0?this.y<e-1?(this.y++,this.lines.pop()):(this.ybase++,this.ydisp++):this.ybase<Math.min(this.lines.maxLength,this.lines.length+o)-e&&(this.ybase===this.ydisp&&this.ydisp++,this.ybase++);',
};

describe('patchXtermBundle', () => {
  for (const [bundle, code] of Object.entries(UNFIXED)) {
    it(`gives ${bundle}'s scroll at the bottom row the same room check as its other branch`, () => {
      const { source, alreadyPatched } = patchXtermBundle(`prefix;${code}suffix`);
      expect(alreadyPatched).toBe(false);
      const guard = code.match(/:(this\.ybase<Math\.min\([^)]*\)-\w)&&/)![1];
      expect(source).toContain(`this.lines.pop()):${guard}&&(this.ybase++,this.ydisp++):${guard}&&`);
      expect(source.startsWith('prefix;')).toBe(true);
      expect(source.endsWith('suffix')).toBe(true);
    });

    it(`leaves ${bundle} as it is when it already has the fix`, () => {
      const fixed = patchXtermBundle(code).source;
      expect(patchXtermBundle(fixed)).toEqual({ source: fixed, alreadyPatched: true });
    });
  }

  it('refuses a bundle without the code, so that a changed xterm fails the install instead of shipping unfixed', () => {
    expect(() => patchXtermBundle('nothing to see')).toThrow(/found it 0 times unfixed and 0 fixed/);
  });

  it('refuses a bundle with the code more than once', () => {
    expect(() => patchXtermBundle(UNFIXED['xterm.js'] + UNFIXED['xterm.js'])).toThrow(/found it 2 times unfixed/);
  });
});
