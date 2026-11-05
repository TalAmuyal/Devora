/** Exercise the signed distribution using the same launcher and bridge as acceptance. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { launch, quit, stop, waitUntilReady } from '../acceptance/support/app';
import { writeTestConfig } from '../../project-ember/tests/support/fixture-helper';

const source = path.resolve(process.argv[2] ?? 'build/bundle/Devora Sapphire.app');
execFileSync('codesign', ['--verify', '--deep', '--strict', source]);
const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'Sapphire bundle smoke '));
try {
  const app = path.join(scratch, 'Installed Sapphire.app');
  execFileSync('ditto', [source, app]);
  const resources = path.join(app, 'Contents/Resources');
  for (const item of ['bundled-apps/debi', 'bundled-apps/ccc', 'bundled-apps/debi-go', 'cc-plugins/judgess/judgess', 'cc-simple-statusline', 'git-shortcuts/gcl', 'themes/catppuccin-macchiato.conf', 'USER_GUIDE.md', 'VERSION', 'BUILD_FINGERPRINT']) {
    assert.ok(fs.existsSync(path.join(resources, item)), `Missing bundle resource: ${item}`);
  }
  assert.equal(fs.readlinkSync(path.join(resources, 'bundled-apps/ccc')), 'debi');
  const version = fs.readFileSync(path.join(resources, 'VERSION'), 'utf8').trim();
  assert.ok(version.length > 0);
  // A unique document in the copy proves F1 reads Resources, not the checkout fallback.
  const marker = `Bundled guide smoke ${Date.now()}`;
  fs.writeFileSync(path.join(resources, 'USER_GUIDE.md'), `# ${marker}\n`);
  execFileSync('codesign', ['--force', '--sign', '-', '--options', 'runtime', app]);
  execFileSync('codesign', ['--verify', '--deep', '--strict', app]);
  const home = path.join(scratch, 'home');
  fs.mkdirSync(home);
  const config = path.join(scratch, 'config.json');
  writeTestConfig(config, []);
  const launched = await launch(config, undefined, { binary: path.join(app, 'Contents/MacOS/Devora Sapphire'), home });
  try {
    await waitUntilReady(launched.bridge);
    const suffixFile = path.join(resources, 'INSTALL_SUFFIX');
    const suffix = fs.existsSync(suffixFile) ? fs.readFileSync(suffixFile, 'utf8').trim() : '';
    assert.ok(fs.existsSync(path.join(home, 'Library/Application Support', `Devora Sapphire${suffix ? ` ${suffix}` : ''}`, 'state.db')), 'Install suffix must reach the real state directory');
    const displayName = execFileSync('/usr/libexec/PlistBuddy', ['-c', 'Print :CFBundleDisplayName', path.join(app, 'Contents/Info.plist')], { encoding: 'utf8' }).trim();
    assert.equal(displayName, `Devora Sapphire${suffix ? `-${suffix}` : version.includes('-dev.') ? `-${version}` : ''}`);
    await launched.bridge.clearWindowSurfaces();
    await launched.bridge.dispatch({ command: 'openShell' });
    const state = await launched.bridge.waitFor('the bundled shell', () => launched.bridge.state(), state => state.tabs.length === 1 && state.tabs[0].terminal === 'running');
    const report = path.join(scratch, 'shell-paths.txt');
    const quote = (s: string) => "'" + s.replaceAll("'", "'\"'\"'") + "'";
    await launched.bridge.typeLine(`{ command -v debi; command -v ccc; command -v debi-go; command -v gcl; } > ${quote(report)}`);
    const lines = await launched.bridge.waitFor('bundled command resolution', async () => fs.existsSync(report) ? fs.readFileSync(report, 'utf8').trimEnd().split('\n') : [], value => value.length === 4);
    // Login shells' path_helper may put system directories first, which is accepted
    // (#277). Check core launch-spec prefixes and real-shell command resolution,
    // without asserting the login shell's PATH order.
    const prefixes = (state.tabs[0].launchPath ?? '').split(':');
    assert.deepEqual(lines, [...['debi', 'ccc', 'debi-go'].map(tool => path.join(resources, 'bundled-apps', tool)), path.join(resources, 'git-shortcuts/gcl')]);
    await launched.bridge.press('F1');
    await launched.bridge.waitFor('the bundled guide via F1', () => launched.bridge.guide(), guide => guide.error === null && guide.text.includes(marker), 30_000);
    assert.deepEqual(await launched.bridge.scrapeErrors(), []);
    assert.deepEqual(prefixes.slice(0, 2), [path.join(resources, 'git-shortcuts'), path.join(resources, 'bundled-apps')]);
    const pluginBins = fs.readdirSync(path.join(resources, 'cc-plugins')).map(plugin => path.join(resources, 'cc-plugins', plugin, 'bin')).filter(bin => fs.existsSync(bin));
    assert.deepEqual(new Set(prefixes.slice(2, 2 + pluginBins.length)), new Set(pluginBins));
    await quit(launched);
    console.log(`Bundle smoke passed: ${displayName}; tools, PATH, state, F1 and signatures`);
  } finally { await stop(launched.app, 'SIGTERM'); }
} finally { fs.rmSync(scratch, { recursive: true, force: true }); }
