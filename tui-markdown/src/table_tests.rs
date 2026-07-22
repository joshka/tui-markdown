mod table {
    use pretty_assertions::assert_eq;

    use super::*;

    #[rstest]
    fn simple_table(_with_tracing: DefaultGuard) {
        let text = from_str(indoc! {"
            | A | B |
            |---|---|
            | 1 | 2 |
        "});
        assert_eq!(text.lines.len(), 5);
        assert!(format!("{}", text.lines[0]).contains('┌'));
    }

    #[rstest]
    fn table_with_alignment(_with_tracing: DefaultGuard) {
        let text = from_str(indoc! {"
            | Left | Center | Right |
            |:-----|:------:|------:|
            | a    | b      | c     |
        "});
        assert_eq!(text.lines.len(), 5);
        assert!(format!("{}", text.lines[2]).contains('┼'));
    }

    #[rstest]
    fn table_with_header_styling(_with_tracing: DefaultGuard) {
        let text = from_str("| Name |\n|------|\n| foo  |");
        let has_bold = text.lines[1]
            .spans
            .iter()
            .any(|span| span.style.add_modifier.contains(ratatui::style::Modifier::BOLD));
        assert!(has_bold);
    }

    #[rstest]
    fn table_after_paragraph(_with_tracing: DefaultGuard) {
        let text = from_str(indoc! {"
            Hello world

            | A | B |
            |---|---|
            | 1 | 2 |
        "});
        assert!(text.lines.len() >= 7);
        assert_eq!(format!("{}", text.lines[0]), "Hello world");
    }

    #[rstest]
    fn table_empty_cells(_with_tracing: DefaultGuard) {
        let text = from_str("| A | B |\n|---|---|\n|   |   |");
        assert_eq!(text.lines.len(), 5);
    }

    #[rstest]
    fn table_with_inline_code(_with_tracing: DefaultGuard) {
        let text = from_str("| Name | Type |\n|------|------|\n| foo  | `u32` |");
        let code_style = Style::new().white().on_black();
        assert!(text.lines[3].spans.iter().any(|span| span.style == code_style));
    }

    #[rstest]
    fn table_with_bold_in_cells(_with_tracing: DefaultGuard) {
        let text = from_str("| Col |\n|-----|\n| **bold** |");
        let has_bold = text.lines[3]
            .spans
            .iter()
            .any(|span| span.style.add_modifier.contains(ratatui::style::Modifier::BOLD));
        assert!(has_bold);
    }

    #[rstest]
    fn table_bottom_border(_with_tracing: DefaultGuard) {
        let text = from_str("| X |\n|---|\n| y |");
        let bottom = format!("{}", text.lines.last().unwrap());
        assert!(bottom.contains('└'));
        assert!(bottom.contains('┘'));
    }

    #[rstest]
    fn table_multi_row(_with_tracing: DefaultGuard) {
        let text = from_str("| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |");
        assert_eq!(text.lines.len(), 6);
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
