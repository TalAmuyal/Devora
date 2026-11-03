// Refuse a partial generated web bundle even when mise's output timestamps are current.
import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve('build/web-renderer');
const manifest = JSON.parse(fs.readFileSync(path.join(root, '.vite/manifest.json'), 'utf8'));
for (const entry of Object.values(manifest)) {
  for (const file of [entry.file, ...(entry.css ?? []), ...(entry.assets ?? [])]) {
    if (!file || !fs.existsSync(path.join(root, file))) throw new Error(`Missing web renderer resource: ${file}`);
  }
}
for (const file of ['index.html', 'bootstrap.js']) {
  if (!fs.existsSync(path.join(root, file))) throw new Error(`Missing web renderer ${file}`);
}
