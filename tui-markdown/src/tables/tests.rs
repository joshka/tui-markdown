use indoc::indoc;
use itertools::Itertools;
use pretty_assertions::assert_eq;
use ratatui_core::style::{Style, Stylize};
use ratatui_core::text::{Line, Span, Text};

use super::*;
use crate::{from_str, from_str_with_options, DefaultStyleSheet, Options, StyleSheet};

#[test]
fn empty_table() {
    let builder = TableBuilder::new(vec![]);
    assert!(builder.render(&DefaultStyleSheet).is_empty());
}

#[test]
fn single_cell() {
    let mut builder = TableBuilder::new(vec![Alignment::None]);
    builder.start_cell();
    builder.push_span(Span::raw("hi"));
    builder.finish_cell();
    builder.finish_header();
    assert_eq!(builder.render(&DefaultStyleSheet).len(), 4);
}

#[test]
fn padding_for_each_alignment() {
    assert_eq!(padding(10, 3, Alignment::Left), (0, 7));
    assert_eq!(padding(10, 3, Alignment::Right), (7, 0));
    assert_eq!(padding(10, 4, Alignment::Center), (3, 3));
    assert_eq!(padding(10, 3, Alignment::Center), (3, 4));
}

#[test]
fn cell_style_covers_padding_and_empty_cells() {
    let style = Style::new().on_green();
    let cell = TableCell {
        spans: vec![Span::raw("x")],
    };
    assert_eq!(
        cell.render_spans(4, Alignment::Center, style),
        [
            Span::styled("  ", style),
            Span::styled("x", style),
            Span::styled("   ", style),
        ]
    );

    let empty_cell = TableCell::default();
    assert_eq!(
        empty_cell.render_spans(4, Alignment::Right, style),
        [Span::styled("     ", style), Span::styled(" ", style)]
    );
}

#[test]
fn column_widths_have_a_minimum_of_one() {
    let mut builder = TableBuilder::new(vec![]);
    builder.header.cells.push(TableCell::default());
    assert_eq!(builder.column_widths(1), vec![1]);
}

#[test]
fn styled_cell_width() {
    let cell = TableCell {
        spans: vec![Span::from("hello").bold(), Span::raw(" world")],
    };
    assert_eq!(cell.width(), 11);
}

#[test]
fn emoji_cell_width() {
    let cell = TableCell {
        spans: vec![Span::raw("✅"), Span::raw(" ok")],
    };
    assert_eq!(cell.width(), 5);
}

#[test]
fn cjk_cell_width() {
    let cell = TableCell {
        spans: vec![Span::raw("日本"), Span::raw(" ok")],
    };
    assert_eq!(cell.width(), 7);
}

#[test]
fn table_with_alignment() {
    let text = from_str(indoc! {"
            | Left | Center | Right |
            |:-----|:------:|------:|
            | a    | b      | c     |
        "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
    assert_eq!(
        rendered,
        [
            "┌──────┬────────┬───────┐",
            "│ Left │ Center │ Right │",
            "├──────┼────────┼───────┤",
            "│ a    │   b    │     c │",
            "└──────┴────────┴───────┘",
        ]
    );
}

#[test]
fn table_without_outer_pipes() {
    let text = from_str(indoc! {"
        A | B
        ---|---
        a | b
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();

    assert_eq!(
        rendered,
        [
            "┌───┬───┐",
            "│ A │ B │",
            "├───┼───┤",
            "│ a │ b │",
            "└───┴───┘",
        ]
    );
}

#[test]
fn escaped_pipe_stays_inside_its_cell() {
    let text = from_str(indoc! {"
        | Value |
        |-------|
        | a \\| b |
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();

    assert_eq!(
        rendered,
        [
            "┌───────┐",
            "│ Value │",
            "├───────┤",
            "│ a | b │",
            "└───────┘",
        ]
    );
}

#[test]
fn table_with_cjk_content() {
    let text = from_str(indoc! {"
            | Latin | CJK |
            |-------|-----|
            | a     | 日本 |
        "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
    assert_eq!(
        rendered,
        [
            "┌───────┬──────┐",
            "│ Latin │ CJK  │",
            "├───────┼──────┤",
            "│ a     │ 日本 │",
            "└───────┴──────┘",
        ]
    );
    assert!(text.lines.iter().all(|line| line.width() == 16));
}

#[derive(Clone)]
struct CustomTableStyleSheet;

impl StyleSheet for CustomTableStyleSheet {
    fn heading(&self, _level: u8) -> Style {
        Style::default()
    }

    fn code(&self) -> Style {
        Style::default()
    }

    fn link(&self) -> Style {
        Style::new().blue().underlined()
    }

    fn blockquote(&self) -> Style {
        Style::default()
    }

    fn heading_meta(&self) -> Style {
        Style::default()
    }

    fn metadata_block(&self) -> Style {
        Style::default()
    }

    fn image_alt(&self) -> Style {
        Style::default()
    }

    fn table_header(&self) -> Style {
        Style::new().on_blue()
    }

    fn table_cell(&self) -> Style {
        Style::new().red().on_green()
    }

    fn table_border(&self) -> Style {
        Style::new().red()
    }
}

#[test]
fn custom_styles_apply_to_header_cells_body_cells_and_borders() {
    let border_style = Style::new().red();
    let header_style = Style::new().on_blue();
    let cell_style = Style::new().red().on_green();
    let options = Options::new(CustomTableStyleSheet);
    let text = from_str_with_options(
        indoc! {"
            | A |
            |---|
            | a |
        "},
        &options,
    );
    assert_eq!(
        text,
        Text::from_iter([
            Line::from(Span::styled("┌───┐", border_style)),
            Line::from_iter([
                Span::styled("│", border_style),
                Span::styled(" ", header_style),
                Span::styled("A", header_style),
                Span::styled(" ", header_style),
                Span::styled("│", border_style),
            ]),
            Line::from(Span::styled("├───┤", border_style)),
            Line::from_iter([
                Span::styled("│", border_style),
                Span::styled(" ", cell_style),
                Span::styled("a", cell_style),
                Span::styled(" ", cell_style),
                Span::styled("│", border_style),
            ]),
            Line::from(Span::styled("└───┘", border_style)),
        ])
    );
}

#[test]
fn custom_cell_style_composes_with_inline_formatting() {
    let options = Options::new(CustomTableStyleSheet);
    let text = from_str_with_options(
        indoc! {"
            | A |
            |---|
            | **bold** |
        "},
        &options,
    );
    let body = &text.lines[3];

    assert!(body
        .spans
        .contains(&Span::styled("bold", Style::new().bold().red().on_green())));
}

#[test]
fn table_cell_style_overrides_conflicting_inline_properties() {
    let options = Options::new(CustomTableStyleSheet);
    let text = from_str_with_options(
        indoc! {"
            | A |
            |---|
            | [docs](url) |
        "},
        &options,
    );
    let link = text.lines[3]
        .spans
        .iter()
        .find(|span| span.content == "docs")
        .expect("link cell content");

    assert_eq!(
        link,
        &Span::styled("docs", Style::new().red().underlined().on_green())
    );
}

#[test]
fn table_preserves_surrounding_paragraph_spacing() {
    let text = from_str(indoc! {"
            Before

            | A |
            |---|
            | a |

            After
        "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
    assert_eq!(
        rendered,
        [
            "Before",
            "",
            "┌───┐",
            "│ A │",
            "├───┤",
            "│ a │",
            "└───┘",
            "",
            "After",
        ]
    );
}

#[test]
fn consecutive_tables_keep_separate_layout_state() {
    let text = from_str(indoc! {"
        | Long |
        |------|
        | value |

        | A |
        |---|
        | b |
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();

    assert_eq!(
        rendered,
        [
            "┌───────┐",
            "│ Long  │",
            "├───────┤",
            "│ value │",
            "└───────┘",
            "",
            "┌───┐",
            "│ A │",
            "├───┤",
            "│ b │",
            "└───┘",
        ]
    );
}

#[test]
fn empty_cells_keep_minimum_column_width() {
    let text = from_str(indoc! {"
        | A | B |
        |---|---|
        |   |   |
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
    assert_eq!(
        rendered,
        [
            "┌───┬───┐",
            "│ A │ B │",
            "├───┼───┤",
            "│   │   │",
            "└───┴───┘",
        ]
    );
}

#[test]
fn header_only_table_has_a_complete_frame() {
    let text = from_str(indoc! {"
        | A |
        |---|
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();

    #[rustfmt::skip]
    let expected = [
        "┌───┐",
        "│ A │",
        "├───┤",
        "└───┘",
    ];
    assert_eq!(rendered, expected);
}

#[test]
fn short_rows_are_padded_and_extra_cells_are_ignored() {
    let text = from_str(indoc! {"
        | A | B |
        |---|---|
        | one |
        | x | y | ignored |
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();

    assert_eq!(
        rendered,
        [
            "┌─────┬───┐",
            "│ A   │ B │",
            "├─────┼───┤",
            "│ one │   │",
            "│ x   │ y │",
            "└─────┴───┘",
        ]
    );
}

#[test]
fn table_with_inline_code() {
    let text = from_str(indoc! {"
        | Name | Type |
        |------|------|
        | foo  | `u32` |
    "});
    let code_style = Style::new().white().on_black();
    let code = text.lines[3]
        .spans
        .iter()
        .find(|span| span.content == "u32")
        .expect("inline code cell content");
    assert_eq!(code, &Span::styled("u32", code_style));
}

#[test]
fn table_with_bold_in_cells() {
    let text = from_str(indoc! {"
        | Col |
        |-----|
        | **bold** |
    "});
    let bold = text.lines[3]
        .spans
        .iter()
        .find(|span| span.content == "bold")
        .expect("bold cell content");
    assert_eq!(bold, &Span::styled("bold", Style::new().bold()));
}

#[test]
fn table_keeps_link_destination_in_cell() {
    let text = from_str(indoc! {"
        | Link |
        |------|
        | [docs](u) |
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
    assert_eq!(
        rendered,
        [
            "┌──────────┐",
            "│ Link     │",
            "├──────────┤",
            "│ docs (u) │",
            "└──────────┘",
        ]
    );
    let link_style = DefaultStyleSheet.link();
    assert_eq!(
        text.lines[3],
        Line::from_iter([
            Span::styled("│", DefaultStyleSheet.table_border()),
            Span::raw(" "),
            Span::styled("docs", link_style),
            Span::raw(" ("),
            Span::styled("u", link_style),
            Span::raw(")"),
            Span::raw(" "),
            Span::styled("│", DefaultStyleSheet.table_border()),
        ])
    );
}

#[test]
fn table_keeps_inline_features_in_cell() {
    let text = from_str(indoc! {"
        | Value |
        |-------|
        | <em>x</em> $y$ |
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
    assert_eq!(
        rendered,
        [
            "┌────────────────┐",
            "│ Value          │",
            "├────────────────┤",
            "│ <em>x</em> $y$ │",
            "└────────────────┘",
        ]
    );

    let row = &text.lines[3];
    assert!(row
        .spans
        .contains(&Span::styled("<em>", DefaultStyleSheet.html())));
    assert!(row
        .spans
        .contains(&Span::styled("$y$", DefaultStyleSheet.math_inline())));
}

#[test]
fn table_routes_inline_content_through_the_active_cell() {
    let text = from_str(indoc! {"
        | Value |
        |-------|
        | **bold** `code` [link](url) <em>x</em> $y$ [^n] |

        [^n]: note
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();

    assert_eq!(rendered[3], "│ bold code link (url) <em>x</em> $y$ [n] │");
}

#[test]
fn block_markers_inside_cells_remain_inline_text() {
    let text = from_str(indoc! {"
        | Value |
        |-------|
        | # heading |
        | > quote |
        | - list |
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();

    assert_eq!(
        rendered,
        [
            "┌───────────┐",
            "│ Value     │",
            "├───────────┤",
            "│ # heading │",
            "│ > quote   │",
            "│ - list    │",
            "└───────────┘",
        ]
    );
}

#[test]
fn table_in_blockquote_keeps_quote_prefix() {
    let text = from_str(indoc! {"
        > | A |
        > |---|
        > | a |
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
    #[rustfmt::skip]
    let expected = [
        "> ┌───┐",
        "> │ A │",
        "> ├───┤",
        "> │ a │",
        "> └───┘",
    ];
    assert_eq!(rendered, expected);
}

#[test]
fn table_list_item_keeps_marker_and_continuation_indent() {
    let text = from_str(indoc! {"
        - | A |
          |---|
          | a |
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
    #[rustfmt::skip]
    let expected = [
        "- ┌───┐",
        "  │ A │",
        "  ├───┤",
        "  │ a │",
        "  └───┘",
    ];
    assert_eq!(rendered, expected);
}

#[test]
fn later_table_in_list_uses_continuation_indent() {
    let text = from_str(indoc! {"
        - | A |
          |---|
          | a |

          | B |
          |---|
          | b |
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
    assert_eq!(
        rendered,
        [
            "- ┌───┐",
            "  │ A │",
            "  ├───┤",
            "  │ a │",
            "  └───┘",
            "",
            "  ┌───┐",
            "  │ B │",
            "  ├───┤",
            "  │ b │",
            "  └───┘",
        ]
    );
}

#[test]
fn ordered_table_list_item_uses_full_marker_width() {
    let text = from_str(indoc! {"
        10. | A |
            |---|
            | a |
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
    assert_eq!(
        rendered,
        [
            "10. ┌───┐",
            "    │ A │",
            "    ├───┤",
            "    │ a │",
            "    └───┘",
        ]
    );
}

#[test]
fn nested_table_list_item_uses_nested_marker_width() {
    let text = from_str(indoc! {"
        - Parent
          - | A |
            |---|
            | a |
    "});
    let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
    assert_eq!(
        rendered,
        [
            "- Parent",
            "    - ┌───┐",
            "      │ A │",
            "      ├───┤",
            "      │ a │",
            "      └───┘",
        ]
    );
}

#[test]
fn table_snapshot() {
    let text = from_str(indoc! {"
            | Name | Value |
            |------|-------|
            | foo  | bar   |
            | baz  | qux   |
        "});
    insta::assert_snapshot!(text);
}
