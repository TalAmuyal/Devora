/**
 * Shared TypeScript shapes for the workspace/profile Tauri command payloads, matching the Rust `#[serde(rename_all = "camelCase")]` structs.
 * Imported by both the Workspace Hub and the Profile Manager so the two views can't drift.
 */

export interface ProfileInfo {
  name: string;
  path: string;
  repoCount: number;
}

export interface RepoInfo {
  name: string;
  path: string;
  source: string; // "registered" | "auto-discovered"
}

export interface WorkspaceInfo {
  id: string;
  path: string;
  taskTitle: string;
  repos: string[];
  active: boolean;
}

/**
 * Progress event streamed over the `create_workspace` / `add_repo_to_workspace` channels.
 * Matches the Rust `CreationEvent` enum (`#[serde(tag = "type", rename_all = "camelCase")]`).
 */
export type CreationEvent =
  | { type: 'step'; label: string }
  | { type: 'log'; line: string }
  | { type: 'done'; workspace: { path: string; name: string } }
  | { type: 'failed'; message: string }
  | { type: 'cancelled' };
