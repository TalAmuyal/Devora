/**
 * Shared git author/committer identity for test repos.
 *
 * Spread into the `env` of `git commit`/`git push` execSync calls so commits
 * have a deterministic identity without depending on the host's git config.
 */
export const GIT_TEST_IDENTITY = {
  GIT_AUTHOR_NAME: 'Test',
  GIT_AUTHOR_EMAIL: 'test@test.local',
  GIT_COMMITTER_NAME: 'Test',
  GIT_COMMITTER_EMAIL: 'test@test.local',
} as const;
