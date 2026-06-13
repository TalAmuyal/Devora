/**
 * `${count} ${word}` with the word pluralized for any count other than 1.
 * English-only; pass an explicit `plural` for irregular words.
 * e.g. pluralize(1, 'repo') → "1 repo"; pluralize(3, 'repo') → "3 repos".
 */
export function pluralize(count: number, singular: string, plural = `${singular}s`): string {
  return `${count} ${count === 1 ? singular : plural}`;
}
