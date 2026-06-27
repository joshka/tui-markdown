//! Comprehensive integration tests for tui-markdown.
//!
//! These tests verify that all supported markdown features render correctly
//! when processed through `tui_markdown::from_str`.

use pretty_assertions::assert_eq;
use ratatui_core::style::{Style, Stylize};
use ratatui_core::text::{Line, Span, Text};

/// Verify the comprehensive fixture file parses without panicking and produces non-empty output.
#[test]
fn comprehensive_fixture_renders() {
    let markdown = include_str!("fixtures/comprehensive.md");
    let text = tui_markdown::from_str(markdown);
    assert!(
        !text.lines.is_empty(),
        "rendered output should not be empty"
    );
    // Spot-check that some known content appears
    let flat: String = text
        .lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    assert!(flat.contains("Heading 1"));
    assert!(flat.contains("bold"));
    assert!(flat.contains("inline code"));
    assert!(flat.contains("Example"));
    assert!(flat.contains("Hello, world!"));
}

/// Empty input produces empty output.
#[test]
fn empty_input() {
    assert_eq!(tui_markdown::from_str(""), Text::default());
}

/// A single word is rendered as a single-line text.
#[test]
fn single_word() {
    assert_eq!(tui_markdown::from_str("hello"), Text::from("hello"));
}

// ---------------------------------------------------------------------------
// Headings
// ---------------------------------------------------------------------------

#[test]
fn heading_levels() {
    let text = tui_markdown::from_str("# H1\n## H2\n### H3");
    let flat: String = text
        .lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    assert!(flat.contains("H1"));
    assert!(flat.contains("H2"));
    assert!(flat.contains("H3"));
}

#[test]
fn heading_with_attributes() {
    let text = tui_markdown::from_str("# Title {#my-id .cls key=val}");
    let flat: String = text
        .lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    assert!(flat.contains("Title"));
    assert!(flat.contains("#my-id"));
}

// ---------------------------------------------------------------------------
// Inline formatting
// ---------------------------------------------------------------------------

#[test]
fn bold() {
    let text = tui_markdown::from_str("**bold**");
    assert_eq!(text, Text::from(Line::from("bold".bold())));
}

#[test]
fn italic() {
    let text = tui_markdown::from_str("*italic*");
    assert_eq!(text, Text::from(Line::from("italic".italic())));
}

#[test]
fn strikethrough() {
    let text = tui_markdown::from_str("~~struck~~");
    assert_eq!(text, Text::from(Line::from("struck".crossed_out())));
}

#[test]
fn inline_code() {
    let text = tui_markdown::from_str("`code`");
    let code_style = Style::new().white().on_black();
    assert_eq!(
        text,
        Text::from(Line::from(Span::styled("code", code_style)))
    );
}

#[test]
fn combined_bold_italic() {
    let text = tui_markdown::from_str("***both***");
    assert_eq!(text, Text::from(Line::from("both".bold().italic())));
}

#[test]
fn subscript() {
    let text = tui_markdown::from_str("H ~2~ O");
    let sub_style = Style::new().dim().italic();
    assert_eq!(
        text,
        Text::from(Line::from_iter([
            Span::from("H "),
            Span::styled("2", sub_style),
            Span::from(" O"),
        ]))
    );
}

#[test]
fn superscript() {
    let text = tui_markdown::from_str("x ^2^ y");
    let sup_style = Style::new().dim().italic();
    assert_eq!(
        text,
        Text::from(Line::from_iter([
            Span::from("x "),
            Span::styled("2", sup_style),
            Span::from(" y"),
        ]))
    );
}

// ---------------------------------------------------------------------------
// Links
// ---------------------------------------------------------------------------

#[test]
fn link() {
    let text = tui_markdown::from_str("[click](https://example.com)");
    let link_style = Style::new().blue().underlined();
    assert_eq!(
        text,
        Text::from(Line::from_iter([
            Span::styled("click", link_style),
            Span::from(" ("),
            Span::styled("https://example.com", link_style),
            Span::from(")"),
        ]))
    );
}

// ---------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------

#[test]
fn image_with_alt() {
    let text = tui_markdown::from_str("![Alt text](url.png)");
    let image_style = Style::new().dim().italic();
    assert_eq!(
        text,
        Text::from(Line::from(Span::styled("[img] Alt text", image_style)))
    );
}

#[test]
fn image_without_alt() {
    let text = tui_markdown::from_str("![](url.png)");
    let image_style = Style::new().dim().italic();
    assert_eq!(
        text,
        Text::from(Line::from(Span::styled("[img] url.png", image_style)))
    );
}

#[test]
fn image_in_paragraph() {
    let text = tui_markdown::from_str("before ![pic](url.png) after");
    let image_style = Style::new().dim().italic();
    assert_eq!(
        text,
        Text::from(Line::from_iter([
            Span::from("before "),
            Span::styled("[img] pic", image_style),
            Span::from(" after"),
        ]))
    );
}

// ---------------------------------------------------------------------------
// Blockquotes
// ---------------------------------------------------------------------------

#[test]
fn blockquote_simple() {
    let bq_style = Style::new().green();
    assert_eq!(
        tui_markdown::from_str("> Hello"),
        Text::from(Line::from_iter(["\u{2502}", " ", "Hello"]).style(bq_style))
    );
}

#[test]
fn blockquote_nested() {
    let text = tui_markdown::from_str("> Outer\n>> Inner");
    let flat: String = text
        .lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    assert!(flat.contains("Outer"));
    assert!(flat.contains("Inner"));
}

// ---------------------------------------------------------------------------
// Lists
// ---------------------------------------------------------------------------

#[test]
fn unordered_list() {
    let text = tui_markdown::from_str("- A\n- B\n- C");
    assert_eq!(
        text,
        Text::from_iter([
            Line::from_iter(["- ", "A"]),
            Line::from_iter(["- ", "B"]),
            Line::from_iter(["- ", "C"]),
        ])
    );
}

#[test]
fn ordered_list() {
    let text = tui_markdown::from_str("1. A\n2. B");
    assert_eq!(
        text,
        Text::from_iter([
            Line::from_iter(["1. ".light_blue(), "A".into()]),
            Line::from_iter(["2. ".light_blue(), "B".into()]),
        ])
    );
}

#[test]
fn nested_list() {
    let text = tui_markdown::from_str("- Parent\n  - Child");
    assert_eq!(
        text,
        Text::from_iter([
            Line::from_iter(["- ", "Parent"]),
            Line::from_iter(["    - ", "Child"]),
        ])
    );
}

#[test]
fn task_list() {
    let text = tui_markdown::from_str("- [ ] Todo\n- [x] Done");
    assert_eq!(
        text,
        Text::from_iter([
            Line::from_iter(["- [ ] ", "Todo"]),
            Line::from_iter(["- [x] ", "Done"]),
        ])
    );
}

// ---------------------------------------------------------------------------
// Code blocks
// ---------------------------------------------------------------------------

#[test]
fn fenced_code_block() {
    let text = tui_markdown::from_str("```\nhello\n```");
    let flat: String = text
        .lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    assert!(flat.contains("hello"));
}

#[test]
fn fenced_code_block_with_lang() {
    let text = tui_markdown::from_str("```rust\nlet x = 1;\n```");
    let flat: String = text
        .lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    assert!(flat.contains("rust") || flat.contains("let"));
}

// ---------------------------------------------------------------------------
// Horizontal rule
// ---------------------------------------------------------------------------

#[test]
fn horizontal_rule() {
    let text = tui_markdown::from_str("above\n\n---\n\nbelow");
    let rule_str = "\u{2500}".repeat(40);
    assert_eq!(
        text,
        Text::from_iter([
            Line::from("above"),
            Line::default(),
            Line::styled(rule_str, Style::new().dark_gray()),
            Line::default(),
            Line::from("below"),
        ])
    );
}

// ---------------------------------------------------------------------------
// Metadata blocks
// ---------------------------------------------------------------------------

#[test]
fn metadata_block() {
    let text = tui_markdown::from_str("---\ntitle: Test\n---\n\nBody");
    let meta_style = Style::new().light_yellow();
    assert_eq!(
        text,
        Text::from_iter([
            Line::from("---").style(meta_style),
            Line::from("title: Test").style(meta_style),
            Line::from("---").style(meta_style),
            Line::default(),
            Line::from("Body"),
        ])
    );
}

// ---------------------------------------------------------------------------
// Paragraphs
// ---------------------------------------------------------------------------

#[test]
fn two_paragraphs() {
    assert_eq!(
        tui_markdown::from_str("First\n\nSecond"),
        Text::from_iter(["First", "", "Second"])
    );
}

#[test]
fn soft_break() {
    assert_eq!(
        tui_markdown::from_str("Line1\nLine2"),
        Text::from(Line::from_iter([
            Span::from("Line1"),
            Span::from(" "),
            Span::from("Line2"),
        ]))
    );
}

// ---------------------------------------------------------------------------
// Complex nesting
// ---------------------------------------------------------------------------

#[test]
fn bold_with_code_inside() {
    let text = tui_markdown::from_str("**bold `code` bold**");
    let flat: String = text
        .lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    assert!(flat.contains("bold"));
    assert!(flat.contains("code"));
}

#[test]
fn list_with_formatting() {
    let text = tui_markdown::from_str("- **bold item**\n- *italic item*");
    let flat: String = text
        .lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    assert!(flat.contains("bold item"));
    assert!(flat.contains("italic item"));
}

// ---------------------------------------------------------------------------
// GFM Alerts
// ---------------------------------------------------------------------------

#[test]
fn gfm_alert_note() {
    let text = tui_markdown::from_str("> [!NOTE]\n> This is a note.");
    let flat = collect_text(&text);
    assert!(flat.contains("Note"), "note alert should render Note label");
}

#[test]
fn gfm_alert_tip() {
    let text = tui_markdown::from_str("> [!TIP]\n> Helpful tip.");
    let flat = collect_text(&text);
    assert!(flat.contains("Tip"), "tip alert should render Tip label");
}

#[test]
fn gfm_alert_important() {
    let text = tui_markdown::from_str("> [!IMPORTANT]\n> Pay attention.");
    let flat = collect_text(&text);
    assert!(
        flat.contains("Important"),
        "important alert should render label"
    );
}

#[test]
fn gfm_alert_warning() {
    let text = tui_markdown::from_str("> [!WARNING]\n> Be careful!");
    let flat = collect_text(&text);
    assert!(
        flat.contains("Warning"),
        "warning alert should render label"
    );
}

#[test]
fn gfm_alert_caution() {
    let text = tui_markdown::from_str("> [!CAUTION]\n> Danger ahead.");
    let flat = collect_text(&text);
    assert!(
        flat.contains("Caution"),
        "caution alert should render label"
    );
}

#[test]
fn fixture_contains_all_alert_types() {
    let markdown = include_str!("fixtures/comprehensive.md");
    let text = tui_markdown::from_str(markdown);
    let flat = collect_text(&text);
    for label in &["Note", "Tip", "Important", "Warning", "Caution"] {
        assert!(flat.contains(label), "fixture should contain {label} alert");
    }
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

#[test]
fn table_content_rendered() {
    let markdown = include_str!("fixtures/comprehensive.md");
    let text = tui_markdown::from_str(markdown);
    let flat = collect_text(&text);
    assert!(flat.contains("Alice"), "table should contain Alice");
    assert!(flat.contains("Bob"), "table should contain Bob");
    assert!(flat.contains("NYC"), "table should contain NYC");
}

#[test]
fn table_border_characters() {
    let text = tui_markdown::from_str("| A |\n|---|\n| B |");
    let flat = collect_text(&text);
    // Table renderer uses box-drawing characters.
    assert!(
        flat.contains('\u{2502}') || flat.contains('|') || flat.contains('\u{2500}'),
        "table should contain border characters"
    );
}

#[test]
fn table_simple() {
    let md = "| X | Y |\n|---|---|\n| 1 | 2 |";
    let text = tui_markdown::from_str(md);
    let flat = collect_text(&text);
    assert!(flat.contains('X'));
    assert!(flat.contains('Y'));
    assert!(flat.contains('1'));
    assert!(flat.contains('2'));
}

// ---------------------------------------------------------------------------
// Math
// ---------------------------------------------------------------------------

#[test]
fn inline_math() {
    let text = tui_markdown::from_str("The formula $E=mc^2$ is famous.");
    let flat = collect_text(&text);
    assert!(flat.contains("E=mc^2"), "inline math should render formula");
}

#[test]
fn display_math() {
    let text = tui_markdown::from_str("$$\nx^2 + y^2 = z^2\n$$");
    let flat = collect_text(&text);
    assert!(flat.contains("x^2"), "display math should render formula");
}

#[test]
fn inline_math_has_separate_delimiters() {
    let text = tui_markdown::from_str("$\\alpha$");
    let line = &text.lines[0];
    // Dollar delimiters are now separate dim spans.
    let dollar_spans: Vec<_> = line
        .spans
        .iter()
        .filter(|s| s.content.as_ref() == "$")
        .collect();
    assert_eq!(
        dollar_spans.len(),
        2,
        "should have two separate $ delimiter spans"
    );
    let content_span = line
        .spans
        .iter()
        .find(|s| s.content.contains("\\alpha"))
        .expect("math content span should exist");
    assert!(
        !content_span.content.starts_with('$'),
        "content span should not contain delimiters"
    );
}

#[test]
fn inline_math_style() {
    use ratatui_core::style::Color;
    let text = tui_markdown::from_str("$E=mc^2$");
    let line = &text.lines[0];
    let math_span = line
        .spans
        .iter()
        .find(|s| s.content.contains("E=mc^2"))
        .expect("math span should exist");
    assert_eq!(
        math_span.style,
        Style::new().italic().fg(Color::Magenta),
        "inline math should use magenta italic style"
    );
}

// ---------------------------------------------------------------------------
// Footnotes
// ---------------------------------------------------------------------------

#[test]
fn footnote_reference() {
    let text = tui_markdown::from_str("Text[^1]\n\n[^1]: Footnote content.");
    let flat = collect_text(&text);
    assert!(
        flat.contains("[1]"),
        "footnote reference should render as [1]"
    );
}

#[test]
fn footnote_definition() {
    let text = tui_markdown::from_str("Text[^note]\n\n[^note]: A named footnote.");
    let flat = collect_text(&text);
    assert!(
        flat.contains("[note]:"),
        "footnote definition should render label"
    );
}

#[test]
fn fixture_has_footnotes() {
    let markdown = include_str!("fixtures/comprehensive.md");
    let text = tui_markdown::from_str(markdown);
    let flat = collect_text(&text);
    assert!(
        flat.contains("[1]") || flat.contains("[note]"),
        "fixture should contain footnote references"
    );
}

// ---------------------------------------------------------------------------
// Definition Lists
// ---------------------------------------------------------------------------

#[test]
fn definition_list_term_and_definition() {
    let text = tui_markdown::from_str("Term 1\n: Definition for term 1\n");
    let flat = collect_text(&text);
    assert!(flat.contains("Term 1"), "should render the term");
    assert!(
        flat.contains("Definition for term 1"),
        "should render the definition"
    );
}

#[test]
fn definition_list_indentation() {
    let text = tui_markdown::from_str("Term\n: Description here\n");
    let line_with_desc = text
        .lines
        .iter()
        .find(|l| l.spans.iter().any(|s| s.content.contains("Description")))
        .expect("should have definition line");
    let has_colon_prefix = line_with_desc
        .spans
        .iter()
        .any(|s| s.content.contains(": "));
    assert!(has_colon_prefix, "definition should have colon prefix");
}

#[test]
fn definition_list_multiple_definitions() {
    let text = tui_markdown::from_str("Term\n: Def A\n: Def B\n");
    let flat = collect_text(&text);
    assert!(flat.contains("Def A"), "first definition");
    assert!(flat.contains("Def B"), "second definition");
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

#[test]
fn html_block_renders() {
    let text = tui_markdown::from_str("<div>Custom HTML</div>\n");
    assert!(!text.lines.is_empty(), "HTML block should produce output");
    let flat = collect_text(&text);
    assert!(
        flat.contains("Custom HTML"),
        "HTML block content should be visible"
    );
}

#[test]
fn inline_html_renders() {
    let text = tui_markdown::from_str("Hello <em>world</em>");
    let line = &text.lines[0];
    assert!(
        line.spans.len() >= 2,
        "inline HTML should produce multiple spans"
    );
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn very_long_line() {
    let long = "word ".repeat(200);
    let text = tui_markdown::from_str(&long);
    assert!(
        !text.lines.is_empty(),
        "very long line should produce output"
    );
}

#[test]
fn deeply_nested_list() {
    let md = "- a\n  - b\n    - c\n      - d\n        - e\n";
    let text = tui_markdown::from_str(md);
    let flat = collect_text(&text);
    assert!(
        flat.contains('e'),
        "deeply nested list should render all levels"
    );
}

#[test]
fn multiple_blank_lines_collapse() {
    let md = "First\n\n\n\n\nSecond";
    let text = tui_markdown::from_str(md);
    let flat = collect_text(&text);
    assert!(flat.contains("First"), "text before blanks");
    assert!(flat.contains("Second"), "text after blanks");
}

// ---------------------------------------------------------------------------
// Fixture-level tests
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_metadata() {
    let markdown = include_str!("fixtures/comprehensive.md");
    let text = tui_markdown::from_str(markdown);
    let flat = collect_text(&text);
    assert!(
        flat.contains("title: Comprehensive Markdown Test"),
        "fixture metadata should be rendered"
    );
}

#[test]
fn fixture_has_images() {
    let markdown = include_str!("fixtures/comprehensive.md");
    let text = tui_markdown::from_str(markdown);
    let flat = collect_text(&text);
    assert!(
        flat.contains("Alt text for image") || flat.contains("[img]"),
        "fixture should contain image rendering"
    );
}

#[test]
fn fixture_has_tables() {
    let markdown = include_str!("fixtures/comprehensive.md");
    let text = tui_markdown::from_str(markdown);
    let flat = collect_text(&text);
    assert!(flat.contains("Alice"), "fixture should contain table data");
    assert!(flat.contains("Bob"), "fixture should contain table data");
}

#[test]
fn fixture_has_math() {
    let markdown = include_str!("fixtures/comprehensive.md");
    let text = tui_markdown::from_str(markdown);
    let flat = collect_text(&text);
    assert!(
        flat.contains("E = mc^2") || flat.contains("E=mc"),
        "fixture should contain inline math"
    );
}

#[test]
fn fixture_has_definition_lists() {
    let markdown = include_str!("fixtures/comprehensive.md");
    let text = tui_markdown::from_str(markdown);
    let flat = collect_text(&text);
    assert!(
        flat.contains("Term 1"),
        "fixture should contain definition list terms"
    );
    assert!(
        flat.contains("Definition for term 1"),
        "fixture should contain definition list descriptions"
    );
}

#[test]
fn fixture_line_count() {
    let markdown = include_str!("fixtures/comprehensive.md");
    let text = tui_markdown::from_str(markdown);
    assert!(
        text.lines.len() > 50,
        "comprehensive fixture should produce many lines, got {}",
        text.lines.len()
    );
}

// ---------------------------------------------------------------------------
// Snapshot tests using insta
// ---------------------------------------------------------------------------

#[test]
fn snapshot_mixed_document() {
    let md = "\
# Title

A paragraph with **bold**, *italic*, and `code`.

- Item 1
- Item 2

> A quote

---

![img](pic.png)

[link](https://example.com)
";
    let text = tui_markdown::from_str(md);
    insta::assert_debug_snapshot!(text);
}

// The comprehensive fixture snapshot differs based on whether syntax highlighting is enabled,
// so we only run it with the default feature set (highlight-code enabled).
#[cfg_attr(not(feature = "highlight-code"), ignore)]
#[test]
fn snapshot_comprehensive_fixture() {
    let markdown = include_str!("fixtures/comprehensive.md");
    let text = tui_markdown::from_str(markdown);
    insta::assert_debug_snapshot!(text);
}

// ---------------------------------------------------------------------------
// Parse API (MarkdownContent)
// ---------------------------------------------------------------------------

#[test]
fn parse_returns_content_blocks() {
    let content = tui_markdown::parse("Hello world");
    assert!(
        !content.blocks.is_empty(),
        "parse should return at least one block"
    );
    match &content.blocks[0] {
        tui_markdown::MarkdownBlock::Text(text) => {
            let flat: String = text
                .lines
                .iter()
                .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
                .collect();
            assert!(flat.contains("Hello world"));
        }
        _ => panic!("expected Text block"),
    }
}

#[test]
fn parse_separates_images_as_blocks() {
    let content = tui_markdown::parse("Before\n\n![photo](pic.png)\n\nAfter");
    let image_count = content
        .blocks
        .iter()
        .filter(|b| matches!(b, tui_markdown::MarkdownBlock::Image { .. }))
        .count();
    assert_eq!(image_count, 1, "should have one image block");

    // Verify the image block has the correct URL and alt text.
    let image = content
        .blocks
        .iter()
        .find_map(|b| match b {
            tui_markdown::MarkdownBlock::Image { url, alt, .. } => Some((url, alt)),
            _ => None,
        })
        .expect("should have an image block");
    assert_eq!(image.0, "pic.png");
    assert_eq!(image.1, "photo");
}

#[test]
fn parse_into_text_flattens_images() {
    let content = tui_markdown::parse("Before\n\n![photo](pic.png)\n\nAfter");
    let text = content.into_text();
    let flat: String = text
        .lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    assert!(flat.contains("Before"), "should contain text before image");
    assert!(flat.contains("After"), "should contain text after image");
    assert!(
        flat.contains("[img]"),
        "flattened image should use [img] indicator"
    );
}

#[test]
fn parse_no_images_returns_single_text_block() {
    let content = tui_markdown::parse("Just text");
    assert_eq!(content.blocks.len(), 1);
    assert!(matches!(
        &content.blocks[0],
        tui_markdown::MarkdownBlock::Text(_)
    ));
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Collect all span content from a Text value into a single string for searching.
fn collect_text(text: &Text<'_>) -> String {
    text.lines
        .iter()
        .flat_map(|line| line.spans.iter().map(|s| s.content.as_ref()))
        .collect()
}
