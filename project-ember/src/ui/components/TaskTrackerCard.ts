/**
 * Task Tracker settings card for the Settings Hub.
 * Edits `task-tracker.provider`, and when the provider resolves to Asana, the Asana ID config keys plus the Asana API token.
 * The IDs are plain config keys (via ConfigCard); the token is an OS-keychain secret (via AsanaTokenRow).
 *
 * The Asana rows key off the *resolved* provider, so a profile inheriting `provider: asana` from User Defaults still shows them.
 */

import { createConfigCard, ConfigFieldSpec, ResolvedMap } from './ConfigCard';
import { createAsanaTokenRow } from './AsanaTokenRow';

export function createTaskTrackerCard(profilePath: string | null): HTMLElement {
  const usesAsana = (resolved: ResolvedMap): boolean =>
    resolved['task-tracker.provider'] === 'asana';

  const fields: ConfigFieldSpec[] = [
    {
      key: 'task-tracker.provider',
      label: 'Provider',
      field: {
        kind: 'enum',
        options: [
          { label: 'Inherit', state: 'default' },
          { label: 'None', state: 'value', value: '' },
          { label: 'Asana', state: 'value', value: 'asana' },
        ],
      },
    },
    {
      key: 'task-tracker.asana.workspace-id',
      label: 'Workspace ID',
      field: { kind: 'text' },
      visibleWhen: usesAsana,
    },
    {
      key: 'task-tracker.asana.project-id',
      label: 'Project ID',
      field: { kind: 'text' },
      visibleWhen: usesAsana,
    },
    {
      key: 'task-tracker.asana.cli-tag',
      label: 'CLI tag',
      field: { kind: 'text' },
      visibleWhen: usesAsana,
    },
    {
      key: 'task-tracker.asana.section-id',
      label: 'Section ID',
      hint: 'optional',
      field: { kind: 'text' },
      visibleWhen: usesAsana,
    },
  ];

  return createConfigCard({
    title: 'Task Tracker',
    profilePath,
    fields,
    extraRows: (resolved) => (usesAsana(resolved) ? [createAsanaTokenRow()] : []),
  });
}
