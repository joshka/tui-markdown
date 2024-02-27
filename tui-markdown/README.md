# Tui-markdown

A library for converting markdown content to a [Ratatui] `Text` value. See [Markdown-reader] for an
example application that uses this library.

[![Crate badge]][tui-markdown]
[![Docs.rs Badge]][API Docs]
[![Deps.rs Badge]][Dependency Status]
[![License Badge]](../LICENSE-MIT)
[![Codecov.io Badge]][Code Coverage]
[![Discord Badge]][Ratatui Discord]

[GitHub Repository] · [API Docs] · [Examples] · [Changelog] · [Contributing]

## Installation

```shell
cargo add tui-markdown
```

## Usage

```rust
let input = "# Heading\n\n**bold**"; // this can come from whereever
let text = tui_markdown::from_str(input);
text.render(area, &mut buf);
```

## Status

Initial implementation - this is very much WIP (see lib.rs `todo!()`s)

- [x] Headings
- [ ] Heading attributes / classes / anchors
- [x] Normal paragraphs
- [x] Block quotes
- [ ] Nested block quotes
- [x] Bold (strong)
- [x] Italic (emphasis)
- [x] Strikethrough
- [x] Ordered lists
- [ ] Unordered lists
- [ ] Code blocks
- [ ] Html
- [ ] Footnotes
- [ ] Tables
- [ ] Linebreak handling
- [ ] Rule
- [ ] Tasklists
- [ ] Links
- [ ] Images
- [ ] Metadata blocks

[tui-markdown]: https://crates.io/crate/tui-markdown
[markdown-reader]: https://crates.io/crate/markdown-reader
[Ratatui]: https://crates.io/crates/ratatui

[Crate badge]: https://img.shields.io/crates/v/tui-markdown?logo=rust&style=for-the-badge
[Docs.rs Badge]: https://img.shields.io/docsrs/tui-markdown?logo=rust&style=for-the-badge
[Deps.rs Badge]: https://deps.rs/repo/github/joshka/tui-markdown/status.svg?path=tui-markdown&style=for-the-badge
[License Badge]: https://img.shields.io/crates/l/tui-markdown?style=for-the-badge
[Codecov.io Badge]: https://img.shields.io/codecov/c/github/joshka/tui-markdown?logo=codecov&style=for-the-badge&token=BAQ8SOKEST
[Discord Badge]: https://img.shields.io/discord/1070692720437383208?label=ratatui+discord&logo=discord&style=for-the-badge

[API Docs]: https://docs.rs/crate/tui-markdown/
[Dependency Status]: https://deps.rs/crate/tui-markdown
[Code Coverage]: https://app.codecov.io/gh/joshka/tui-markdown
[Ratatui Discord]: https://discord.gg/pMCEU9hNEj

[GitHub Repository]: https://github.com/joshka/tui-markdown
[Changelog]: https://github.com/joshka/tui-markdown/blob/main/tui-markdown/CHANGELOG.md
[Contributing]: https://github.com/joshka/tui-markdown/blob/main/CONTRIBUTING.md
