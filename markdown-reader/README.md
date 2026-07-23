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

Usage: mdr [OPTIONS] [PATH]

Arguments:
  [PATH]
          The path to the markdown file to read

          [default: README.md]

Options:
      --image-fallback <MODE>
          Text to display in place of Markdown images

          Possible values:
          - alt-text:         Display the image description, falling back to its URL
          - url:              Display the image URL
          - alt-text-and-url: Display the image description and URL

          [default: alt-text]

      --code-theme <THEME>
          Built-in syntax-highlighting theme (default: base16-ocean-dark)

          Possible values:
          - base16-eighties-dark: The dark Base16 Eighties theme
          - base16-mocha-dark:    The dark Base16 Mocha theme
          - base16-ocean-dark:    The default dark Base16 Ocean theme
          - base16-ocean-light:   The light Base16 Ocean theme
          - inspired-github:      The light Inspired GitHub theme
          - solarized-dark:       The dark Solarized theme
          - solarized-light:      The light Solarized theme

      --code-theme-file <PATH>
          Load a custom syntax-highlighting theme from a TextMate .tmTheme file

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

By default, images display their alt text and code uses the Base16 Ocean dark theme. Select a
different representation for images or a built-in code theme:

```shell
mdr --image-fallback alt-text-and-url README.md
mdr --code-theme solarized-dark README.md
```

Custom syntax-highlighting themes can be loaded from TextMate `.tmTheme` files:

```shell
mdr --code-theme-file themes/custom.tmTheme README.md
```

`--code-theme` and `--code-theme-file` are mutually exclusive.

The repository includes a [feature showcase](TEST.md) for manually checking every supported
Markdown construct:

```shell
cargo run -p markdown-reader -- markdown-reader/TEST.md
```

## Status

This is working code, but not every markdown feature is supported. PRs welcome!

- [x] Headings
- [x] Heading attributes / classes / anchors
- [x] Normal paragraphs
- [x] Block quotes
- [x] Nested block quotes
- [x] GFM alerts
- [x] Bold (strong)
- [x] Italic (emphasis)
- [x] Strikethrough
- [x] Ordered lists
- [x] Unordered lists
- [x] Code blocks
- [x] HTML
- [x] Math
- [x] Footnotes
- [x] Definition lists
- [x] Tables
- [x] Linebreak handling
- [x] Rule
- [x] Tasklists
- [x] Links
- [x] Images
- [x] Metadata blocks
- [x] Superscript
- [x] Subscript

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
