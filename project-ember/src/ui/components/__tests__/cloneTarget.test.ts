import { describe, it, expect } from 'vitest';
import { deriveCloneDirName } from '../cloneTarget';

describe('deriveCloneDirName', () => {
  it('derives the repo from GitHub HTTPS URLs', () => {
    expect(deriveCloneDirName('https://github.com/org/repo.git')).toBe('repo');
    expect(deriveCloneDirName('https://github.com/org/repo')).toBe('repo');
    expect(deriveCloneDirName('https://github.com/org/repo/')).toBe('repo');
    expect(deriveCloneDirName('https://github.com/org/repo/pulls')).toBe('repo');
    expect(deriveCloneDirName('https://github.com/org/repo#readme')).toBe('repo');
    expect(deriveCloneDirName('https://github.com/org/repo?tab=x')).toBe('repo');
    expect(deriveCloneDirName('https://www.github.com/org/repo')).toBe('repo');
  });

  it('derives the repo from SSH and other URL forms', () => {
    expect(deriveCloneDirName('git@github.com:org/repo.git')).toBe('repo');
    expect(deriveCloneDirName('git@github.com:org/repo')).toBe('repo');
    expect(deriveCloneDirName('ssh://git@github.com/org/repo.git')).toBe('repo');
    expect(deriveCloneDirName('file:///var/tmp/fixtures/repo.git')).toBe('repo');
    expect(deriveCloneDirName('https://ghe.corp.example/team/repo.git')).toBe('repo');
  });

  it('returns null for empty or unrecognized input', () => {
    for (const input of ['', '   ', 'not a url', 'org/repo', 'github.com/org/repo', 'https://github.com/org']) {
      expect(deriveCloneDirName(input)).toBeNull();
    }
  });
});
