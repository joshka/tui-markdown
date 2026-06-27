// Table rendering integration tests -- included into the tests module of lib.rs
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
        let top = format!("{}", text.lines[0]);
        assert!(top.contains('\u{250C}'));
        assert!(top.contains('\u{252C}'));
        assert!(top.contains('\u{2510}'));
    }

    #[rstest]
    fn table_with_alignment(_with_tracing: DefaultGuard) {
        let text = from_str(indoc! {"
            | Left | Center | Right |
            |:-----|:------:|------:|
            | a    | b      | c     |
        "});
        assert_eq!(text.lines.len(), 5);
        let sep = format!("{}", text.lines[2]);
        assert!(sep.contains('\u{251C}'));
        assert!(sep.contains('\u{253C}'));
        assert!(sep.contains('\u{2524}'));
    }

    #[rstest]
    fn table_with_header_styling(_with_tracing: DefaultGuard) {
        let text = from_str(indoc! {"
            | Name |
            |------|
            | foo  |
        "});
        let header_line = &text.lines[1];
        let has_bold = header_line
            .spans
            .iter()
            .any(|s| s.style.add_modifier.contains(ratatui::style::Modifier::BOLD));
        assert!(has_bold, "Header row should have bold style");
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
        let text = from_str(indoc! {"
            | A | B |
            |---|---|
            |   |   |
        "});
        assert_eq!(text.lines.len(), 5);
    }

    #[rstest]
    fn table_with_inline_code(_with_tracing: DefaultGuard) {
        let text = from_str(indoc! {"
            | Name | Type |
            |------|------|
            | foo  | `u32` |
        "});
        assert_eq!(text.lines.len(), 5);
        let data_row = &text.lines[3];
        let code_style = Style::new().white().on_black();
        let has_code = data_row.spans.iter().any(|s| s.style == code_style);
        assert!(has_code, "Table cell should contain code-styled span");
    }

    #[rstest]
    fn table_with_bold_in_cells(_with_tracing: DefaultGuard) {
        let text = from_str(indoc! {"
            | Col |
            |-----|
            | **bold** |
        "});
        assert_eq!(text.lines.len(), 5);
        let data_row = &text.lines[3];
        let has_bold = data_row
            .spans
            .iter()
            .any(|s| s.style.add_modifier.contains(ratatui::style::Modifier::BOLD));
        assert!(has_bold, "Table cell should contain bold-styled span");
    }

    #[rstest]
    fn table_bottom_border(_with_tracing: DefaultGuard) {
        let text = from_str(indoc! {"
            | X |
            |---|
            | y |
        "});
        let bottom = format!("{}", text.lines.last().unwrap());
        assert!(bottom.contains('\u{2514}'));
        assert!(bottom.contains('\u{2518}'));
    }

    #[rstest]
    fn table_multi_row(_with_tracing: DefaultGuard) {
        let text = from_str(indoc! {"
            | A | B |
            |---|---|
            | 1 | 2 |
            | 3 | 4 |
        "});
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
