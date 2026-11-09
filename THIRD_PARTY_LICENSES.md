# Third-Party Licenses

Devora bundles and depends on the following third-party software and assets.

## Go Dependencies

### charm.land/lipgloss/v2

- **License:** MIT
- **Source:** https://github.com/charmbracelet/lipgloss

### github.com/BurntSushi/toml

- **License:** MIT
- **Source:** https://github.com/BurntSushi/toml

### github.com/goccy/go-yaml

- **License:** MIT
- **Source:** https://github.com/goccy/go-yaml

### golang.org/x/sync

- **License:** BSD-3-Clause
- **Source:** https://github.com/golang/sync

## Assets

### Catppuccin palette

- **License:** MIT
- **Source:** https://github.com/catppuccin/catppuccin
- **Note:** Source of the Mocha hex values used in `project-debi/internal/style/styles.go`.

### Catppuccin for Kitty (Pocco81)

- **License:** MIT
- **Source:** https://github.com/catppuccin/kitty
- **Note:** The Macchiato variant in `themes/catppuccin-macchiato.conf` is taken from this port.

## Devora Sapphire

### Ghostty

- **License:** MIT
- **Source:** https://github.com/ghostty-org/ghostty
- **Note:** libghostty is built from source at a pinned commit and linked into Devora Sapphire; `project-sapphire/Sources/TerminalKit/TerminalView.swift` adapts the keyboard and IME handling of Ghostty's macOS app.

### swift-markdown

- **License:** Apache 2.0
- **Source:** https://github.com/swiftlang/swift-markdown (version 0.9.0, pinned in `project-sapphire/Package.resolved`)
- **Note:** linked into Devora Sapphire to parse the agent transcript's Markdown (ADR-014). Its NOTICE file reads:

> The Swift Markdown Project
>
> Please visit the Swift Markdown web site for more information:
>
> * https://github.com/apple/swift-markdown
>
> Copyright (c) 2021 Apple Inc. and the Swift project authors
>
> The Swift Project licenses this file to you under the Apache License, version 2.0 (the "License"); you may not use this file except in compliance with the License. You may obtain a copy of the License at:
>
> https://www.apache.org/licenses/LICENSE-2.0
>
> Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the specific language governing permissions and limitations under the License.
>
> This product contains Swift Argument Parser (Apache License 2.0, https://github.com/apple/swift-argument-parser).
>
> This product contains a derivation of the cmark-gfm project, available at https://github.com/apple/swift-cmark (BSD-2, https://github.com/github/cmark-gfm).

Swift Argument Parser is used only by swift-markdown's command-line tool, which Devora Sapphire doesn't build or ship.

### swift-cmark

- **License:** BSD 2-clause (Copyright (c) 2014, John MacFarlane), with MIT-licensed parts derived from houdini (Copyright (C) 2012 Vicent Martí), Git (Copyright 2012 GitHub, Inc.) and utf8proc (Copyright 2009 Public Software Group e. V.)
- **Source:** https://github.com/swiftlang/swift-cmark (version 0.9.0, pinned in `project-sapphire/Package.resolved`)
- **Note:** linked into Devora Sapphire through swift-markdown; its `COPYING` file has the full texts.
