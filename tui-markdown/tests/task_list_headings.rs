use ratatui_core::style::{Color, Modifier, Style};
use rstest::rstest;
use tui_markdown::{from_str, from_str_with_options, Options, StyleSheet};

#[derive(Clone, Copy)]
struct HeadingStyles {
    hidden: bool,
}

impl StyleSheet for HeadingStyles {
    fn heading_marker(&self, level: u8) -> &str {
        if self.hidden {
            ""
        } else if level == 1 {
            "#"
        } else {
            "##"
        }
    }

    fn heading(&self, _: u8) -> Style {
        Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    }
}

#[rstest]
#[case::unordered("-", "  ")]
#[case::ordered("1.", "   ")]
#[case::multi_digit_ordered("10.", "    ")]
fn task_list_setext_heading_keeps_checkbox_text_and_style(
    #[case] list_marker: &str,
    #[case] indent: &str,
    #[values("[ ]", "[x]")] checkbox: &str,
    #[values("---", "===")] underline: &str,
    #[values(false, true)] hidden: bool,
) {
    let source = format!("{list_marker} {checkbox} task\n{indent}{underline}");
    let styles = HeadingStyles { hidden };
    let level = if underline == "===" { 1 } else { 2 };
    let marker = styles.heading_marker(level);
    let prefix = if hidden {
        String::new()
    } else {
        format!("{marker} ")
    };
    let text = from_str_with_options(&source, &Options::new(styles));

    assert_eq!(
        text.to_string(),
        format!("{list_marker} \n{prefix}{checkbox} task")
    );
    assert_eq!(text.lines[1].style, styles.heading(level));
    assert_eq!(
        text.lines[1].spans[usize::from(!hidden)].content,
        format!("{checkbox} ")
    );
}

#[rstest]
#[case::unchecked("- [ ] task\n  ---", "- \n## [ ] task")]
#[case::checked("- [x] task\n  ===", "- \n# [x] task")]
fn default_heading_markers_keep_existing_task_output(#[case] source: &str, #[case] expected: &str) {
    assert_eq!(from_str(source).to_string(), expected);
}

#[rstest]
fn ordinary_tasks_keep_markers_and_nested_indentation(#[values(false, true)] hidden: bool) {
    let options = Options::new(HeadingStyles { hidden });
    assert_eq!(
        from_str_with_options("- [ ] first\n  - [x] nested\n- [x] last", &options).to_string(),
        "- [ ] first\n    - [x] nested\n- [x] last"
    );
    assert_eq!(
        from_str_with_options("1. [ ] first\n2. [x] last", &options).to_string(),
        "1. [ ] first\n2. [x] last"
    );
}

#[test]
fn nested_task_heading_can_hide_its_marker() {
    let options = Options::new(HeadingStyles { hidden: true });
    let source = "- outer\n  - [x] task\n    ---";
    assert_eq!(
        from_str_with_options(source, &options).to_string(),
        "- outer\n    - \n[x] task"
    );
}
