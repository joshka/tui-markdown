# Tui-markdown

An experimental Proof of Concept library for converting markdown content to a [Ratatui] `Text`
value. See [Markdown-reader] for an example application that uses this library.

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
let input = "# Heading\n\n**bold**"; // this can come from wherever
let text = tui_markdown::from_str(input);
text.render(area, &mut buf);
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
- [x] Linebreak handling
- [x] Rule
- [x] Tables
- [x] Tasklists
- [x] Links
- [ ] Images
- [x] Metadata blocks
- [x] Superscript
- [x] Subscript

Linebreaks are rendered with Markdown defaults: soft breaks become spaces, hard breaks insert a
new line.

Links are rendered as `label (URL)`. The link style applies to both the visible label and URL while
preserving nested inline formatting such as bold text.

GFM alerts render a bold icon and canonical English label above their quoted body. Customize each
kind's color with [`StyleSheet::alert()`], its terminal-friendly icon with
[`StyleSheet::alert_icon()`], and its label with [`StyleSheet::alert_label()`]. Returning an empty
icon or label displays only the other component.

Raw inline HTML tags and HTML blocks are displayed literally rather than interpreted as terminal
markup. They are dimmed by default and can be customized with [`StyleSheet::html()`].

Inline and display math keep their `$...$` and `$$...$$` delimiters visible. Inline math is
magenta and italic by default, while display math is magenta and preserves multiline formulas as
separate terminal lines. Customize these styles with [`StyleSheet::math_inline()`] and
[`StyleSheet::math_display()`].

Footnote references such as `[^source]` are displayed as `[source]`, and definitions are displayed
as `[source]: ...`. References are dim and italic by default, while definitions are dim. Customize
these styles with [`StyleSheet::footnote_ref()`] and [`StyleSheet::footnote_def()`].

Definition-list terms are bold by default, with each description rendered on its own line after a
colon-and-space prefix. Customize them with [`StyleSheet::definition_term()`] and
[`StyleSheet::definition_description()`].

Metadata blocks are rendered using the metadata block style so front matter is visible, including
the delimiter lines (for example `---` in YAML-style blocks).

```rust
use ratatui::text::Text;
use tui_markdown::from_str;

let markdown = r#"---
title: Demo
tags:
  - one
  - two
---

Body
"#;

let text = from_str(markdown);
assert_eq!(
    text,
    Text::from_iter([
        "---".into(),
        "title: Demo".into(),
        "tags:".into(),
        "  - one".into(),
        "  - two".into(),
        "---".into(),
        "".into(),
        "Body".into(),
    ])
);
```

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
[`StyleSheet::html()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.html
[`StyleSheet::math_display()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.math_display
[`StyleSheet::math_inline()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.math_inline
[`StyleSheet::footnote_def()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.footnote_def
[`StyleSheet::footnote_ref()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.footnote_ref
[`StyleSheet::definition_description()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.definition_description
[`StyleSheet::definition_term()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.definition_term
[`StyleSheet::alert()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.alert
[`StyleSheet::alert_icon()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.alert_icon
[`StyleSheet::alert_label()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.alert_label

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
