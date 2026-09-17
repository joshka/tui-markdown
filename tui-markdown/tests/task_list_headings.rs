use ratatui_core::style::{Color, Modifier, Style};
use tui_markdown::{from_str_with_options, ChangeReason, Options, StreamingMarkdown, StyleSheet};

#[derive(Clone)]
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

#[test]
fn task_list_setext_headings_allow_empty_heading_markers() {
    for (list_marker, indent) in [("-", "  "), ("1.", "   ")] {
        for checkbox in ["[ ]", "[x]"] {
            for (underline, heading_marker) in [("---", "##"), ("===", "#")] {
                for hidden in [false, true] {
                    let source = format!("{list_marker} {checkbox} task\n{indent}{underline}");
                    let options = Options::new(HeadingStyles { hidden });
                    let text = from_str_with_options(&source, &options);
                    let expected_heading = if hidden {
                        format!("{checkbox} task")
                    } else {
                        format!("{heading_marker} {checkbox} task")
                    };
                    assert_eq!(
                        text.to_string(),
                        format!("{list_marker} \n{expected_heading}")
                    );
                    assert_eq!(text.lines[1].style, options_style());
                    assert_eq!(
                        text.lines[1].spans[usize::from(!hidden)].content,
                        format!("{checkbox} ")
                    );
                }
            }
        }
    }
}

fn options_style() -> Style {
    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
}

#[test]
fn ordinary_tasks_keep_markers_text_and_nested_indentation() {
    let source = "- [ ] first\n  - [x] nested\n- [x] last";
    for hidden in [false, true] {
        let options = Options::new(HeadingStyles { hidden });
        assert_eq!(
            from_str_with_options(source, &options).to_string(),
            "- [ ] first\n    - [x] nested\n- [x] last"
        );
        assert_eq!(
            from_str_with_options("1. [ ] first\n2. [x] last", &options).to_string(),
            "1. [ ] first\n2. [x] last"
        );
    }
}

#[test]
fn task_heading_prefixes_and_resizes_match_batch_without_panicking() {
    let corpus = [
        "- [ ] task\n  ---",
        "- [x] task\n  ===",
        "1. [ ] task\n   ---",
        "10. [x] task\n    ===",
        "- outer\n  - [x] task\n    ---",
        "> - [ ] task\n>   ---",
        "- [ ] **task**\r\n  ---",
        "- [ ] task\n---",
    ];
    for hidden in [false, true] {
        for source in corpus {
            for width in [None, Some(0), Some(1), Some(7), Some(40)] {
                let options = Options::new(HeadingStyles { hidden }).width(width);
                let mut stream = StreamingMarkdown::new(options.clone());
                for (start, ch) in source.char_indices() {
                    let end = start + ch.len_utf8();
                    stream.append(&source[start..end]);
                    assert_eq!(stream.source(), &source[..end]);
                    assert_eq!(
                        stream.current(),
                        &from_str_with_options(&source[..end], &options),
                        "hidden={hidden}, width={width:?}, prefix={:?}",
                        &source[..end]
                    );
                }
                stream.finish();
                assert_eq!(stream.current(), &from_str_with_options(source, &options));
                let before = stream.counters();
                assert_eq!(stream.finish().reason, ChangeReason::None);
                assert_eq!(stream.counters(), before);
                for next_width in [Some(0), Some(40), None, Some(7)] {
                    stream.set_width(next_width);
                    assert_eq!(
                        stream.current(),
                        &from_str_with_options(source, &options.clone().width(next_width))
                    );
                }
            }
        }
    }
}

#[test]
fn task_heading_reclassification_preserves_confirmed_prefix_and_later_replay() {
    let options = Options::new(HeadingStyles { hidden: true }).width(Some(40));
    let confirmed = "# confirmed\n\n";
    let source = format!("{confirmed}- [ ] task\n  -");
    let mut stream = StreamingMarkdown::new(options.clone());
    stream.append(confirmed);
    stream.append("- [ ] task\n  ");
    let before = stream.counters();
    stream.append("-");
    assert_eq!(stream.current(), &from_str_with_options(&source, &options));
    assert!((stream.counters() - before).processed_source_bytes < source.len() as u64);
    assert_eq!(stream.current().lines[0].to_string(), "confirmed");

    stream.append("--\n\nAfter");
    let before = stream.counters();
    let update = stream.append(" tail");
    assert!(update.replay_start >= source.len());
    assert!((stream.counters() - before).processed_source_bytes < 32);
    assert_eq!(
        stream.current(),
        &from_str_with_options(stream.source(), &options)
    );
}
