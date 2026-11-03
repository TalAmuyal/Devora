# Shared web renderer

Ember and Sapphire use this package for Markdown and Mermaid.
`markdownToHtml` sanitizes rendered HTML; `renderMermaidDiagrams` loads Mermaid only for documents that contain diagram fences, and leaves a failed diagram's source visible.
Ember mounts the result in its existing content overlay; Sapphire packages the standalone Vite build and serves it through a restricted local WKWebView scheme.
The renderer signals readiness to the host before receiving its content and theme tokens; it never fetches them from the network.
`mise run -C project-sapphire web-renderer` builds the standalone bundle and `scripts/check-web-renderer.mjs` verifies its generated assets before packaging.
