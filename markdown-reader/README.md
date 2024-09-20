# Markdown Reader

[![Crate badge]][markdown-reader]
[![Deps.rs Badge]][Dependency Status]
[![License Badge]](../LICENSE-MIT)
[![Codecov.io Badge]][Code Coverage]
[![Discord Badge]][Ratatui Discord]

[GitHub Repository] · [Changelog] · [Contributing]

An experimental Proof of Concept markdown reader that uses [Ratatui] to render markdown files. The
primary purpose of this crate is to test the [tui-markdown] crate. It is not ready for any sort of
real world use.

![Demo](https://vhs.charm.sh/vhs-160G5PeWh0TMoxBph87WXZ.gif)

## Installation

To install the markdown reader application (mdr):

```shell
cargo install --locked markdown-reader
```

## Usage

```shell
mdr --help

A simple markdown reader that uses ratatui to render markdown files.

Usage: mdr [PATH]

Arguments:
  [PATH]  The path to the markdown file to read [default: README.md]

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## Status

Initial implementation - this is very much WIP (see lib.rs `todo!()`s)

- [x] Headings
- [ ] Heading attributes / classes / anchors
- [x] Normal paragraphs
- [x] Block quotes
- [x] Nested block quotes
- [x] Bold (strong)
- [x] Italic (emphasis)
- [x] Strikethrough
- [x] Ordered lists
- [x] Unordered lists
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

## License

Copyright (c) 2024 Josh McKinney

This project is licensed under either of

- Apache License, Version 2.0
   ([LICENSE-APACHE](../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license
   ([LICENSE-MIT](../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

See [CONTRIBUTING.md](../CONTRIBUTING.md).

[tui-markdown]: https://crates.io/crates/tui-markdown
[markdown-reader]: https://crates.io/crates/markdown-reader
[Ratatui]: https://crates.io/crates/ratatui

[Crate badge]: https://img.shields.io/crates/v/markdown-reader?logo=rust&style=for-the-badge
[Deps.rs Badge]: https://deps.rs/repo/github/joshka/tui-markdown/status.svg?path=markdown-reader&style=for-the-badge
[License Badge]: https://img.shields.io/crates/l/markdown-reader?style=for-the-badge
[Codecov.io Badge]: https://img.shields.io/codecov/c/github/joshka/markdown-reader?logo=codecov&style=for-the-badge&token=BAQ8SOKEST
[Discord Badge]: https://img.shields.io/discord/1070692720437383208?label=ratatui+discord&logo=discord&style=for-the-badge

[Dependency Status]: https://deps.rs/crate/markdown-reader
[Code Coverage]: https://app.codecov.io/gh/joshka/tui-markdown
[Ratatui Discord]: https://discord.gg/pMCEU9hNEj

[GitHub Repository]: https://github.com/joshka/tui-markdown
[Changelog]: https://github.com/joshka/tui-markdown/blob/main/markdown-reader/CHANGELOG.md
[Contributing]: https://github.com/joshka/tui-markdown/blob/main/CONTRIBUTING.md
