# Shared web renderer

Ember and Sapphire use this package for Markdown and Mermaid.
`markdownToHtml` sanitizes rendered HTML; `renderMermaidDiagrams` loads Mermaid only for documents that contain diagram fences, and leaves a failed diagram's source visible.
Ember mounts the result in its existing content overlay; Sapphire packages the standalone Vite build and serves it through a restricted local WKWebView scheme.
The renderer signals readiness to the host before receiving its content and theme tokens; it never fetches them from the network.
`mise run -C project-sapphire web-renderer` builds the standalone bundle and `scripts/check-web-renderer.mjs` verifies its generated assets before packaging.

The standalone entry point also renders typed agent transcripts with `devoraRenderChat`.
`chat.ts` keeps sequence-keyed rows and morphs sanitized Markdown while preserving unchanged text nodes and expanded thinking/tool details.
Tool inputs, results, system events and permission descriptions are inserted as text.
Only host-created permission buttons send control messages, carrying the observed generation and request ID.
Scroll messages carry the top visible sequence and offset, or null while following the bottom.
History requests include that anchor before the native host prepends a page.
