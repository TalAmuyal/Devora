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
