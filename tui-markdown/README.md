# Tui-markdown

An experimental Proof of Concept library for converting markdown content to a [Ratatui] `Text`
value. See [Markdown-reader] for an example application that uses this library.

[![Crate badge]][tui-markdown]
[![Docs.rs Badge]][API Docs]
[![Deps.rs Badge]][Dependency Status]
[![License Badge]](LICENSE-MIT)
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

### Width-aware rendering

Use the opt-in `from_str_with_context` API to prepare rows for the available body width. Exclude
application-owned reply markers and other surrounding UI from this width:

```rust
use tui_markdown::{from_str_with_context, Options, RenderContext};

let context = RenderContext::new(1)
    .with_wide_grapheme_replacement('*')
    .expect("a printable ASCII character");
let text = from_str_with_context("\u{754c}", &Options::default(), &context);
assert_eq!(text.to_string(), "*");
```

Normal line-end overflow wraps to another row without changing the text. Only a whole grapheme
that is wider than the entire body width uses a replacement, which defaults to `-`. The replacement
retains the text style and must be one printable ASCII character (`U+0020` through `U+007E`);
non-ASCII and control characters return `InvalidReplacementCharacter`. Source remains unchanged,
so rendering at a wider width restores the original grapheme. Width zero returns no rows.

Tables use stacked rows and numbered cells when their grid cannot fit or its configurable
`TableLimits` are exceeded. These limits cover grid-presentation buffers, not all parser or output
allocations. Joined cell text used only for cross-style grapheme measurement is charged to that
table buffer as one reusable per-table scratch allocation; an over-budget table switches to stacked
presentation before allocating the scratch. The complete current snapshot is accounted separately
by `ResourceUsage`. The original `from_str` and `from_str_with_options` APIs keep their unwrapped
behavior.

### Streaming rendering

`StreamingMarkdown` retains exact source and an owned `Text<'static>` snapshot. Ordinary appends
replay only the parser-confirmed mutable suffix; `finish` performs one fresh canonical
whole-document pass for each source version:

```rust
use tui_markdown::{Options, RenderContext, StreamingMarkdown};

let mut markdown = StreamingMarkdown::new(Options::default(), RenderContext::new(80));
markdown.append("First paragraph.\n\n");
let update = markdown.append("Second **paragraph**.");

let complete_text = markdown.current();
let earliest_replacement = update.first_changed_row;
let irreversible_prefix = update.stable_rows;
assert_eq!(markdown.source(), "First paragraph.\n\nSecond **paragraph**.");

markdown.finish();
```

Retained UIs can borrow only the currently visible rows plus bounded overscan without cloning or
walking the rest of the snapshot:

```rust
# use tui_markdown::{Options, RenderContext, StreamingMarkdown};
# let mut markdown = StreamingMarkdown::new(Options::default(), RenderContext::new(80));
# markdown.append("one\n\ntwo\n\nthree");
let viewport = markdown.prepare_rows(1, 2);
for row in viewport.rows() {
    // Draw, measure, select, and hit-test this same prepared row.
    let _ = row;
}
```

Repeated viewport requests reuse the same row storage and perform no parser or renderer work. The
borrow prevents mutation while a prepared view is active, avoiding stale geometry. The complete
snapshot remains available through `current`; applications remain responsible for retaining only
bounded active/history objects.

`replace`, `clear`, `set_context`, and `set_options` invalidate incompatible state. Repeated
`current`, empty `append`, unchanged `set_context`, and repeated `finish` do no parser or renderer
work. `WorkCounters` exposes source-free processing counts, while `ResourceUsage` accounts for
source and owned snapshots without claiming a total-memory cap. References and footnotes use an
explicit conservative whole-document recomputation path and never make a false stable-prefix
promise. A nonempty append after `finish` explicitly starts a new mutable lineage and returns
`ChangeReason::Reopen`; it never reports an ordinary append after all rows were declared stable.

### Syntax highlighting themes

With the default `highlight-code` feature enabled, fenced code blocks whose language is recognized
use the built-in `Base16OceanDark` syntax-highlighting theme. Pass a [`BuiltinCodeTheme`] to select
a different bundled theme:

```rust
use tui_markdown::{from_str_with_options, BuiltinCodeTheme, Options};

let options = Options::default().code_theme(BuiltinCodeTheme::InspiredGitHub);
let markdown = r#"```rust
fn main() {}
```"#;
let text = from_str_with_options(markdown, &options);
```

[`CodeTheme::from_file`] reads and parses a TextMate `.tmTheme` file immediately. The returned theme
owns the parsed data, so rendering does not access the file again:

```rust
use tui_markdown::{CodeTheme, CodeThemeLoadError, Options};

fn options() -> Result<Options, CodeThemeLoadError> {
    let theme = CodeTheme::from_file("themes/solarized.tmTheme")?;
    Ok(Options::default().code_theme(theme))
}
```

Use [`CodeTheme::from_textmate`] with [`include_str!`] to compile a theme into the application
instead of reading it at runtime. The returned theme owns the parsed data and does not borrow the
source string:

```rust
use tui_markdown::{CodeTheme, CodeThemeLoadError, Options};

fn options() -> Result<Options, CodeThemeLoadError> {
    let source = include_str!("../themes/my-theme.tmTheme");
    let theme = CodeTheme::from_textmate(source)?;
    Ok(Options::default().code_theme(theme))
}
```

[`CodeTheme::from_file`]: https://docs.rs/tui-markdown/latest/tui_markdown/struct.CodeTheme.html#method.from_file
[`CodeTheme::from_textmate`]: https://docs.rs/tui-markdown/latest/tui_markdown/struct.CodeTheme.html#method.from_textmate
[`include_str!`]: https://doc.rust-lang.org/std/macro.include_str.html
[`BuiltinCodeTheme`]: https://docs.rs/tui-markdown/latest/tui_markdown/enum.BuiltinCodeTheme.html

### Markdown presentation symbols

The renderer normally retains heading markers and frames block code with triple backticks. A custom
style sheet can replace either symbol with [`StyleSheet::heading_marker()`] and
[`StyleSheet::code_block_fence()`], or return an empty string to hide it:

```rust
use ratatui::style::Style;
use tui_markdown::{from_str_with_options, DefaultStyleSheet, Options, StyleSheet};

#[derive(Clone, Copy)]
struct MinimalStyleSheet;

impl StyleSheet for MinimalStyleSheet {
    fn heading(&self, level: u8) -> Style {
        DefaultStyleSheet.heading(level)
    }

    fn code(&self) -> Style {
        DefaultStyleSheet.code()
    }

    fn link(&self) -> Style {
        DefaultStyleSheet.link()
    }

    fn blockquote(&self) -> Style {
        DefaultStyleSheet.blockquote()
    }

    fn heading_meta(&self) -> Style {
        DefaultStyleSheet.heading_meta()
    }

    fn metadata_block(&self) -> Style {
        DefaultStyleSheet.metadata_block()
    }

    fn heading_marker(&self, _level: u8) -> &str {
        ""
    }

    fn code_block_fence(&self) -> &str {
        ""
    }
}

let options = Options::new(MinimalStyleSheet);
let markdown = "# Heading\n\n```text\ncode\n```";
let text = from_str_with_options(markdown, &options);

assert_eq!(text.to_string(), "Heading\n\ncode");
```

The code-block fence choice is independent of syntax highlighting and applies to fenced and
indented code blocks alike. Other presentation symbols, such as list markers, blockquote prefixes,
image indicators, and table borders, retain their standard output.

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
- [x] Images
- [x] Metadata blocks
- [x] Superscript
- [x] Subscript

Linebreaks are rendered with Markdown defaults: soft breaks become spaces, hard breaks insert a
new line.

Images render as text fallbacks rather than terminal graphics. The default output uses `[img]`
followed by the image description, or the destination when the description is empty. For example,
`Before ![diagram](diagram.png) after` renders as `Before [img] diagram after`.

Use [`ImageFallback`] to show the destination instead, or to include it after the description:

```rust
use tui_markdown::{from_str_with_options, ImageFallback, Options};

let options = Options::default().image_fallback(ImageFallback::AltTextAndUrl);
let text = from_str_with_options("![diagram](diagram.png)", &options);
assert_eq!(text.to_string(), "[img] diagram (diagram.png)");
```

GFM tables render with Unicode box-drawing borders and honor left, center, and right column
alignment:

```markdown
| Name | Status |
|:-----|-------:|
| API  | Ready  |
```

```text
┌──────┬────────┐
│ Name │ Status │
├──────┼────────┤
│ API  │  Ready │
└──────┴────────┘
```

Column widths use terminal display width, so wide CJK and emoji characters remain aligned. Use
[`StyleSheet::table_header()`] for header cells, [`StyleSheet::table_cell()`] for body cells, and
[`StyleSheet::table_border()`] for the box-drawing borders. Cell styles cover content and padding
while preserving inline formatting unless they set the same style property.

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
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

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
[`StyleSheet::table_border()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.table_border
[`StyleSheet::table_cell()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.table_cell
[`StyleSheet::table_header()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.table_header
[`StyleSheet::heading_marker()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.heading_marker
[`StyleSheet::code_block_fence()`]: https://docs.rs/tui-markdown/latest/tui_markdown/trait.StyleSheet.html#method.code_block_fence
[`ImageFallback`]: https://docs.rs/tui-markdown/latest/tui_markdown/enum.ImageFallback.html

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
