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

### Width-aware rendering

Use `Options::width` to fit rendered text into the space available in your UI.
Pass the options to `from_str_with_options` when rendering a complete string,
or to `StreamingMarkdown::new` when receiving text in pieces.

```rust
use tui_markdown::{from_str_with_options, Options};

let options = Options::default().width(Some(4));
let text = from_str_with_options("abcdefghij", &options);
assert_eq!(text.to_string(), "abcd\nefgh\nij");
```

- Width is measured in terminal cells (columns), not pixels or bytes.
  Most ASCII characters occupy one cell; many CJK characters and emoji occupy two.
- Supply the width available for Markdown text. Exclude your UI's reply markers and borders.
  For example, `Some(80)` allows 80 cells per row; it is not a default or a required width.
- `None` is the default. The package still renders Markdown, but it does not wrap long lines
  to a width. Your UI decides how to display them.
- `Some(0)` returns no display rows. It does not mean unlimited width.
- The package does not read the terminal size. Call `StreamingMarkdown::set_width` when it changes.

#### Characters that do not fit

Text that does not fit at the end of a row moves to the next row. Wrapping keeps each grapheme
together: a grapheme is one displayed character, such as a letter with an accent or a joined emoji.

If a grapheme is wider than the **entire** available row, the renderer shows `-` instead.
Use `Options::with_wide_grapheme_replacement` to choose another printable ASCII character:

```rust
use tui_markdown::{from_str_with_options, Options};

let options = Options::default()
    .width(Some(1))
    .with_wide_grapheme_replacement('*')
    .expect("a printable ASCII character");
let text = from_str_with_options("\u{754c}", &options);
assert_eq!(text.to_string(), "*");
```

The replacement keeps the original style. Non-ASCII characters and control characters return
`InvalidReplacementCharacter`; printable ASCII means `U+0020` through `U+007E`.
The original source is never changed. Rendering at a wider width restores the original character.

#### Tables and buffer limits

With `Options::width(Some(...))`, tables stay as bordered grids when possible.
Wide cells wrap into multiple physical lines. Horizontal rules separate logical rows.
Each cell has one space of padding on each side. Shorter cells are padded to their row's height.
Left, right, and center alignment apply to each physical cell line.

The renderer keeps natural column widths when they fit. Otherwise, it caps wider columns
while retaining shorter columns. Remaining space is assigned from left to right.
Each column keeps at least one terminal cell and enough space for its widest complete grapheme.
This prevents narrow columns from replacing CJK characters or emoji.
List indentation and quote prefixes are subtracted before fitting the grid.

If even these minimum column widths, padding, and borders cannot fit, the renderer lists
each row's cells vertically with numbers. It keeps every cell's content instead of cutting it off.
Widening a streaming document rebuilds the grid from the original source.

- `Options::table_limits` controls how much data the renderer buffers while building a table grid.
  Defaults are 8,192 logical cells and 4 MiB of tracked buffer capacity.
  Headers and empty cells count toward the cell limit.
- If a limit prevents building the grid, the renderer uses the same vertical presentation.
  Setting either limit to zero requests this presentation for every table.
- These limits apply only when a width is set. With `width(None)`, the original table behavior
  remains in effect, even if you supplied custom limits.
- The limits do not cap total memory use. The source, rendered output, and parser also use memory.

The renderer sometimes joins styled cell fragments to measure and wrap graphemes that cross
between them. It counts this shared buffer and both column-width vectors against the byte limit.
It checks the limit before allocating and reuses the joined-content buffer across cells.
Wrapping writes directly into the rendered rows without a second buffer of wrapped cells.
`ResourceUsage` separately reports the stored rendered output.

### Streaming rendering

Use `StreamingMarkdown` when Markdown arrives in pieces, such as an agent's reply.
The object stores the source and updates its rendered output as you append text.

- Call `append(&str)` with each **new** fragment, in order. Do not send the accumulated source again.
- Call `current()` after an update to read the complete current output, not just the latest fragment.
- Call `finish()` when input ends.
- Create a separate object for each independent document or response.

```rust
use tui_markdown::{Options, StreamingMarkdown};

let mut markdown = StreamingMarkdown::new(Options::default().width(Some(80)));
markdown.append("First paragraph.\n\n");
let update = markdown.append("Second **paragraph**.");

assert_eq!(markdown.current().to_string(), "First paragraph.\n\nSecond paragraph.");
assert!(update.first_changed_row.is_some());
assert_eq!(markdown.source(), "First paragraph.\n\nSecond **paragraph**.");
markdown.finish();
```

Each fragment must be valid UTF-8. It may end inside Markdown syntax or between code points
of a grapheme. The caller handles network decoding, input order, and when text becomes visible.

Updates finish before the method returns. The result is Ratatui `Text`, containing `Line` and
styled `Span` values. It is not HTML, an image, or terminal escape codes. Your UI draws the result.
Later input can still change earlier output, for example when it closes unfinished emphasis.

#### Changing settings

Batch and streaming accept the same `Options`.

- Use `set_width`, `set_table_limits`, or `set_wide_grapheme_replacement` to change layout
  without replacing your styles. Passing the same value does no work.
- Without tables, a layout change rearranges the cached output without parsing the source again.
  With tables, the renderer processes the full source to rebuild their layout.
- An invalid replacement character returns an error and leaves the document unchanged.
- Use `set_options` to replace the complete options. This always processes the full source.
  Custom style sheets need not support equality checks, so the method does not assume they match.
- Use `replace` to supply a different complete source, or `clear` to reuse the object for new text.

#### Reading a display-row range

Use `prepare_rows(first_row, row_count)` when your UI needs only part of the rendered output.
It returns a read-only view of the stored rows without reprocessing Markdown or copying rows.

- `first_row` is zero-based. `row_count` is a count, not an end index.
- Rows refer to the output, including any width wrapping, not to source lines.
  Human-numbered rows 10 through 20 use `prepare_rows(9, 11)`.
- A range past the end returns only available rows. A start at or beyond the end returns no rows.
- The method does not know which rows are visible in your window. Your UI chooses the range.

```rust
use tui_markdown::{Options, StreamingMarkdown};

let mut markdown = StreamingMarkdown::new(Options::default().width(Some(80)));
markdown.append("one\n\ntwo\n\nthree");
let viewport = markdown.prepare_rows(1, 2);
assert_eq!(viewport.first_row(), 1);
assert_eq!(viewport.rows(), &markdown.current().lines[1..3]);
assert!(markdown.prepare_rows(100, 2).is_empty());
```

Both `current()` and `prepare_rows()` reuse stored output. Reading it does not parse, render,
allocate, or clone. The UI still draws those rows and handles text selection and mouse clicks.
Using the same rows for these tasks keeps mouse positions aligned with displayed text.

Rust prevents document updates while borrowed rows are still in use.
The `'static` in `Text<'static>` describes the owned text content, not the lifetime of your reference.
The full output is prepared before you read it; selecting a range does not limit initial rendering
to that range. Your application decides how many document objects and past replies to keep.

#### How incremental updates work

The object tracks where parsing needs to resume. This position is its **replay checkpoint**.
You do not maintain it yourself.

- Ordinary `append` reuses output before the checkpoint. It parses and renders the affected
  suffix: the source from that position onward.
- An unfinished paragraph, list, table, or code block may need to be processed again.
  Incremental rendering does not mean processing only the latest fragment.
- If the checkpoint is still at byte zero, the suffix is the whole current document.
- References and footnotes can affect text outside that suffix. These use the full-source
  processing paths below.

Every current result must match rendering the same received source from scratch with the same
options. `finish()` does not repair otherwise incorrect intermediate results.

#### When the renderer processes the full source

**`finish()` is not the only full-source operation.** The current behavior is:

| Operation or condition | Parsing and rendering work |
| --- | --- |
| Ordinary nonempty `append` | Start at the replay checkpoint, which may be zero |
| A reference, footnote, or other document-wide dependency is found | A full-source pass may be needed, including for unresolved references |
| Later nonempty appends after a document-wide dependency was found | Keep processing the full source until `replace` supplies different source or `clear` removes it |
| `replace` with different, nonempty source | Parse and render the new source in full |
| `set_options` | Parse and render the full source, even if the new options would give the same output |
| A changed layout-only setter, without tables | Rearrange cached output without parsing again |
| A changed layout-only setter, with tables | Parse and render the full source to rebuild table layout |
| `finish` | Parse and render the full source once for the current source and options |
| Nonempty `append` after `finish` | Start accepting text again, process the full source, and report `ChangeReason::Reopen` |

The following do no parser or renderer work:

- `current()` and `prepare_rows()`.
- Empty `append`, `replace` with the same source, or a layout-only setter with the same value.
- Repeated `finish` with no source or options change.
- `clear` or `replace("")`. They remove source and stored output; an already empty document is unchanged.

#### Using update information

Document updates report `Update` information:

- `first_changed_row` gives the first output row that differs, or `None` if the output is unchanged.
  A UI can use it to decide where redrawing must start.
- `stable_rows` counts the initial rows that later ordinary appends will not change.
  Use this count if you write to output that cannot be revised.
- `replay_start` gives the original UTF-8 byte offset where processing began.
- `reason` tells you which operation or dependency caused the update.

Already rendered rows are not necessarily stable. The stability promise applies only while
appending to the same document with unchanged options. Replacement, clearing, layout changes,
and `ChangeReason::Reopen` start a new period; do not carry the old promise across them.

Use `counters()` to measure parser and renderer work without recording the source.
The returned `WorkCounters` includes processed bytes, events, and `layout_reflows`.
`full_recomputations` counts explicit full-source operations; ordinary replay from byte zero is
not counted there. Use byte and event counts to measure total work.

Use `resource_usage()` to inspect the source and output buffers the object keeps.
`ResourceUsage` reports tracked storage, not a total-memory limit.

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
