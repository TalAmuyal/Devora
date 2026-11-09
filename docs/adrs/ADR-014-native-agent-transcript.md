# ADR-014: Native Agent Transcript

## Status

Accepted.

## Date

2026-11-09

## Context

ADR-012 rendered an Agent Space's chat transcript through the shared web renderer (a `WKWebView` that previews and the User Guide also use), with a native renderer as the fallback if that proved better.
A time-boxed spike built a throwaway native transcript in Sapphire and measured it against the web one on the same synthetic rows, in release builds.

- At 200 rows (the tail the chat shows while following the bottom), native opened in about 400 ms against 500 ms, and painted a streaming update in about 33 ms against 49 ms.
- At 5,000 rows, the web transcript's cost per update grew with the transcript (about 630 ms), while native's stayed at about 33 ms.
- Native used about 150–300 MB less memory, with no WebContent process; the web one's grew during a run, even at 200 rows.
- Both kept the bottom in view while following it, and kept the visible row in place while streaming and while older pages loaded.
- Native's scrolling stalled for about 400 ms per batch of rows coming into view, because `NSTableView` asks for many row heights at once and measuring a hosted SwiftUI row costs about a millisecond.
- SwiftUI's text selection works within one text view, not across a reply's paragraphs or across rows.
- Diagrams (Mermaid) still need a web view to draw.

## Decision

The transcript becomes native: an `NSTableView` whose rows host SwiftUI views, fed `AgentChat`'s items.

- Row heights are estimated until measured: the visible rows are measured at once, and the rest a little at a time on idle frames, nearest first, until every row is exact; then nothing runs.
	A measurement that changes a height keeps what the reader sees in place: the bottom while following it, otherwise the top visible row.
- Markdown is parsed by `swift-markdown` (Apache 2.0, over `swift-cmark`, BSD 2-clause), Sapphire's first external package, into plain values that the rows draw.
- Selection and Copy are per row; selecting across rows is shelved.
- Mermaid diagrams are drawn by one shared, offscreen web renderer into images.
- The test bridge observes the transcript from its view and model rather than from a web page.

It lands in three steps: the table behind a hidden switch, with the web transcript still the default; parity for every row kind, the permission cards, Mermaid and the bridge; then the native transcript as the only one, removing the web transcript's code.
The web renderer stays for previews and the User Guide.

### Alternatives

- **Keep the web transcript**, and make its updates incremental instead of rendering the whole transcript each time.
	It was the spike's recommendation: at 200 rows native gains little, and it costs several weeks, cross-row selection and a second Mermaid path.
	Native was chosen for its memory, its constant cost per update on long transcripts, and a transcript drawn like the rest of the app, which the bubbles, spinner and timestamps that follow build on.
- **An `NSTextView` (TextKit 2) per row**, which would allow selecting across a reply's paragraphs.
	Deferred: hosted SwiftUI rows measured reliably and are simpler to lay out; it remains the route if cross-paragraph selection becomes a requirement.

## Consequences

- Sapphire depends on a Swift package fetched at build time, pinned by `Package.resolved` for both `swift build` and the generated app target; its licenses are in `THIRD_PARTY_LICENSES.md`.
- Every row kind (thinking, tools, permission cards, system rows) is native work, and so is each change to the transcript's look.
- The transcript keeps WebKit only for Mermaid diagrams.
- Current behavior is documented in `project-sapphire/CLAUDE.md`.
