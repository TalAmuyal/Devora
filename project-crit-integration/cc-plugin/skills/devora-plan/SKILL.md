---
name: devora-plan
description: Author an expressive plan with embedded HTML that renders cleanly in Crit (Devora)
user-invocable: true
disable-model-invocation: true
argument-hint: What to plan (a task or feature description)
---
Write a plan for the task described below. Beyond plain markdown, embed HTML (and Mermaid
diagrams) wherever they make the plan clearer — and style any HTML with Crit's design tokens so
it renders seamlessly in Crit's plan view, in both light and dark themes.

$ARGUMENTS

## How Crit renders a plan

Crit shows the plan in a web view. Knowing how it renders keeps your HTML predictable:

- Plans are parsed by **markdown-it with `html: true` and no sanitizer**, so raw HTML,
  `style="…"`, and `class="…"` attributes all pass through verbatim. (Safe because Crit only
  reviews your own local files.)
- Rendered content is wrapped in a `.line-content` container, and Crit already styles
  **standard** elements through it — headings, paragraphs, links, inline `code`, `pre`, tables,
  blockquotes, lists, images, `hr`. **A bare element already matches the theme; you rarely need
  to style it.**
- Two elements get **no** Crit styling and fall back to browser defaults: `<details>/<summary>`
  and a bare `<div>`. Style those yourself with tokens (below) if you want them on-theme.
- Fenced **` ```mermaid `** code blocks render as **diagrams**, and they follow the active
  light/dark theme automatically.
- A `<script>` tag **will execute** — never embed scripts. A relative `<img src="…">` is
  rewritten to `/files/…`.

## When to add HTML or a diagram

Default to plain markdown. Add an HTML element or a diagram only when it makes the plan easier
to grasp, e.g.:

- A **callout** to highlight a risk, decision, or constraint.
- A **status badge** to label items (done / blocked / proposed).
- A **diagram** (Mermaid) for architecture, a flow, or a decision tree.
- A **comparison** of options, side by side.
- **Color-coded** inline states inside dense text.
- A **collapsible** section to tuck away long detail (sample payloads, logs).

If a bullet list says it just as clearly, use the bullet list.

## Expressive toolbox

Copy-paste starting points — replace the placeholder content. Block-level HTML needs a blank
line before and after it; inline HTML can sit inside a paragraph.

### Mermaid diagram

```mermaid
flowchart LR
  A[Request] --> B{Cached?}
  B -- yes --> C[Return cached]
  B -- no --> D[Fetch + store] --> C
```

### Callout / admonition

<div style="background: var(--crit-brand-bg); border: 1px solid var(--crit-border); border-left: 3px solid var(--crit-brand); border-radius: var(--crit-r-lg); padding: 12px 16px;">
  <strong style="color: var(--crit-brand);">Decision</strong><br/>
  We will do X because Y.
</div>

Swap `--crit-brand*` for `--crit-orange` (warning), `--crit-red` (danger), or `--crit-green`
(success) to recolor the callout.

### Status badge / pill

<span style="background: var(--crit-green); color: var(--crit-editor-bg); padding: 2px 10px; border-radius: var(--crit-r-sm); font-family: var(--crit-font-mono); font-size: 0.85em;">done</span>

### Comparison table

A bare table already matches Crit. Add tokens only for a branded header:

<table style="width:100%; border-collapse: collapse;">
  <thead>
    <tr>
      <th style="text-align:left; padding:10px 14px; background: var(--crit-brand-bg); color: var(--crit-brand); border-bottom: 2px solid var(--crit-border);">Option</th>
      <th style="text-align:left; padding:10px 14px; background: var(--crit-brand-bg); color: var(--crit-brand); border-bottom: 2px solid var(--crit-border);">Trade-off</th>
    </tr>
  </thead>
  <tbody>
    <tr><td style="padding:8px 14px; border-bottom:1px solid var(--crit-border);">A</td><td style="padding:8px 14px; border-bottom:1px solid var(--crit-border);">simple, slower</td></tr>
    <tr style="background: var(--crit-table-stripe);"><td style="padding:8px 14px;">B</td><td style="padding:8px 14px;">faster, more code</td></tr>
  </tbody>
</table>

### Side-by-side layout

<div style="display:flex; gap:12px;">
  <div style="flex:1; border:1px solid var(--crit-border); border-radius: var(--crit-r-md); padding:12px;">
    <strong style="color: var(--crit-brand);">Before</strong><br/>…
  </div>
  <div style="flex:1; border:1px solid var(--crit-border); border-radius: var(--crit-r-md); padding:12px;">
    <strong style="color: var(--crit-brand);">After</strong><br/>…
  </div>
</div>

### Collapsible detail

<details style="border:1px solid var(--crit-border); border-radius: var(--crit-r-md); padding:8px 12px; background: var(--crit-editor-bg-card);">
  <summary style="cursor:pointer; color: var(--crit-brand); font-weight:600;">Sample payload</summary>
  <div style="margin-top:8px; color: var(--crit-editor-fg-secondary);">…long detail…</div>
</details>

### Inline color-coded status

<span style="color: var(--crit-green);">ready</span>, <span style="color: var(--crit-orange);">in review</span>, <span style="color: var(--crit-red);">blocked</span>.

## Styling rules (match Crit, don't fight it)

1. **Bare standard elements already match the theme** — don't style tables, code, blockquotes,
   links, lists, or headings unless you genuinely need a variant.
2. **Never hardcode colors.** `#888` / `red` / `#f6f6f6` clash with the theme and break when the
   user switches light/dark. Always use `var(--crit-*)` tokens — they're defined for both themes
   and adapt automatically.
3. **Block HTML needs blank lines** before and after; inline HTML can live inside a paragraph.
4. Crit's button classes work in plan HTML: `class="btn"`, `class="btn btn-primary"`,
   `class="btn btn-sm"`, `class="btn btn-danger"`.

## Crit token reference

Use these by name (values live in Crit's `theme.css` and track the active theme):

| Purpose | Tokens |
| --- | --- |
| Text | `--crit-editor-fg`, `--crit-editor-fg-secondary`, `--crit-editor-fg-muted` |
| Surfaces | `--crit-editor-bg`, `--crit-editor-bg-card`, `--crit-editor-bg-elevated`, `--crit-editor-code-bg` |
| Accent / brand | `--crit-brand`, `--crit-brand-bg`, `--crit-brand-subtle` |
| Borders | `--crit-border`, `--crit-border-strong` |
| Semantic | `--crit-green`, `--crit-red`, `--crit-orange`, `--crit-yellow`, `--crit-purple` |
| Blockquote | `--crit-blockquote-border`, `--crit-blockquote-bg` |
| Table stripe | `--crit-table-stripe` |
| Radius | `--crit-r-sm` (4px), `--crit-r-md` (6px), `--crit-r-lg` (8px), `--crit-r-xl` (12px) |
| Fonts | `--crit-font-body`, `--crit-font-mono` |

## Before you finish

Review the plan in Crit and toggle light/dark once — every styled element should adapt with no
hardcoded color leaking through. If something looks off, it's almost always a hardcoded value
that should be a `var(--crit-*)` token.
