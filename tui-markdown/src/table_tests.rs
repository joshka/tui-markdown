mod table {
    use pretty_assertions::assert_eq;

    use super::*;

    #[rstest]
    fn table_with_alignment(_with_tracing: DefaultGuard) {
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

    #[rstest]
    fn table_with_cjk_content(_with_tracing: DefaultGuard) {
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
            Style::default()
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

        fn table_header(&self) -> Style {
            Style::new().on_blue()
        }

        fn table_border(&self) -> Style {
            Style::new().red()
        }
    }

    #[rstest]
    fn custom_styles_apply_to_header_content_and_every_border(_with_tracing: DefaultGuard) {
        let border_style = Style::new().red();
        let header_style = Style::new().on_blue();
        let options = Options::new(CustomTableStyleSheet);
        let text = from_str_with_options("| A |\n|---|\n| a |", &options);
        assert_eq!(
            text,
            Text::from_iter([
                Line::from(Span::styled("┌───┐", border_style)),
                Line::from_iter([
                    Span::styled("│", border_style),
                    Span::raw(" "),
                    Span::styled("A", header_style),
                    Span::raw(" "),
                    Span::styled("│", border_style),
                ]),
                Line::from(Span::styled("├───┤", border_style)),
                Line::from_iter([
                    Span::styled("│", border_style),
                    Span::raw(" "),
                    Span::raw("a"),
                    Span::raw(" "),
                    Span::styled("│", border_style),
                ]),
                Line::from(Span::styled("└───┘", border_style)),
            ])
        );
    }

    #[rstest]
    fn table_preserves_surrounding_paragraph_spacing(_with_tracing: DefaultGuard) {
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
                "Before", "", "┌───┐", "│ A │", "├───┤", "│ a │", "└───┘", "", "After",
            ]
        );
    }

    #[rstest]
    fn empty_cells_keep_minimum_column_width(_with_tracing: DefaultGuard) {
        let text = from_str("| A | B |\n|---|---|\n|   |   |");
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

    #[rstest]
    fn table_with_inline_code(_with_tracing: DefaultGuard) {
        let text = from_str("| Name | Type |\n|------|------|\n| foo  | `u32` |");
        let code_style = Style::new().white().on_black();
        let code = text.lines[3]
            .spans
            .iter()
            .find(|span| span.content == "u32")
            .expect("inline code cell content");
        assert_eq!(code, &Span::styled("u32", code_style));
    }

    #[rstest]
    fn table_with_bold_in_cells(_with_tracing: DefaultGuard) {
        let text = from_str("| Col |\n|-----|\n| **bold** |");
        let bold = text.lines[3]
            .spans
            .iter()
            .find(|span| span.content == "bold")
            .expect("bold cell content");
        assert_eq!(bold, &Span::styled("bold", Style::new().bold()));
    }

    #[rstest]
    fn table_keeps_link_destination_in_cell(_with_tracing: DefaultGuard) {
        let text = from_str("| Link |\n|------|\n| [docs](u) |");
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

    #[rstest]
    fn table_keeps_inline_features_in_cell(_with_tracing: DefaultGuard) {
        let text = from_str("| Value |\n|-------|\n| <em>x</em> $y$ |");
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
        assert!(row.spans.contains(&Span::styled(
            "$y$",
            DefaultStyleSheet.math_inline()
        )));
    }

    #[rstest]
    fn table_in_blockquote_keeps_quote_prefix(_with_tracing: DefaultGuard) {
        let text = from_str("> | A |\n> |---|\n> | a |");
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
        assert_eq!(
            rendered,
            [
                "> ┌───┐",
                "> │ A │",
                "> ├───┤",
                "> │ a │",
                "> └───┘",
            ]
        );
    }

    #[rstest]
    fn table_snapshot(_with_tracing: DefaultGuard) {
        let text = from_str(indoc! {"
            | Name | Value |
            |------|-------|
            | foo  | bar   |
            | baz  | qux   |
        "});
        insta::assert_snapshot!(text);
    }
}
