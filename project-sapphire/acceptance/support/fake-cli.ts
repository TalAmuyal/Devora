import * as fs from 'node:fs';
import * as path from 'node:path';
import type { SapphireWorld } from './world';

/**
 * The fake CLI's agent launches, in order, as it reports them (`app.ts` puts its report beside the config): each one's
 * arguments. Not the model list's probes (initialize-only launches that save no session), which the New Agent dialog and
 * the Claude card start whenever they open.
 */
export function agentLaunches(world: SapphireWorld): string[][] {
  const report = fs.readFileSync(path.join(path.dirname(world.testConfigPath), 'fake-claude.jsonl'), 'utf8');
  return report.split('\n').filter(Boolean).map((line) => JSON.parse(line))
    .filter((entry) => entry.kind === 'start' && !entry.args.includes('--no-session-persistence'))
    .map((entry) => entry.args);
}
