use ratatui_core::style::{Color, Modifier, Style};
use ratatui_core::text::Text;
use tui_markdown::{from_str_with_options, Options, StreamingMarkdown, StyleSheet, TableLimits};

const TABLE: &str = "| ID | Detail |\n| - | - |\n| x | alpha beta gamma |\n| y | abcdefghijk |";

fn render(source: &str, width: u16) -> Text<'_> {
    from_str_with_options(source, &Options::default().width(Some(width)))
}

fn assert_grid(text: &Text<'_>, width: usize) {
    assert!(text.to_string().starts_with('┌'), "{text}");
    assert!(text.to_string().ends_with('┘'), "{text}");
    assert!(
        text.lines.iter().all(|line| line.width() == width),
        "{text}"
    );
}

#[test]
fn narrow_grid_wraps_words_and_long_tokens_and_separates_logical_rows() {
    let text = render(TABLE, 18);
    assert_eq!(
        text.to_string(),
        [
            "┌────┬───────────┐",
            "│ ID │ Detail    │",
            "├────┼───────────┤",
            "│ x  │ alpha     │",
            "│    │ beta      │",
            "│    │ gamma     │",
            "├────┼───────────┤",
            "│ y  │ abcdefghi │",
            "│    │ jk        │",
            "└────┴───────────┘",
        ]
        .join("\n")
    );
    assert_grid(&text, 18);
}

#[test]
fn wide_grid_retains_natural_columns_and_none_retains_legacy_rows() {
    let wide = render(TABLE, 80);
    assert_eq!(
        wide.to_string(),
        [
            "┌────┬──────────────────┐",
            "│ ID │ Detail           │",
            "├────┼──────────────────┤",
            "│ x  │ alpha beta gamma │",
            "├────┼──────────────────┤",
            "│ y  │ abcdefghijk      │",
            "└────┴──────────────────┘",
        ]
        .join("\n")
    );
    assert_grid(&wide, 25);
    let legacy = from_str_with_options(TABLE, &Options::default());
    assert_eq!(
        legacy.to_string(),
        wide.lines
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != 4)
            .map(|(_, line)| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn short_keys_stay_intact_with_words_long_tokens_and_unicode_after_resizing() {
    let words = "alpha beta gamma delta epsilon zeta eta theta";
    let token = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let unicode = "界界 café e\u{301} 👩‍💻";
    let source = format!(
        "| Key | Description |\n| - | - |\n| Item01 | {words} |\n| Item02 | {token} |\n| Item03 | {unicode} |"
    );
    let expected = [
        vec!["Key", "Description"],
        vec!["Item01", words],
        vec!["Item02", token],
        vec!["Item03", unicode],
    ];
    let mut stream = StreamingMarkdown::new(Options::default().width(Some(80)));
    stream.append(&source);
    stream.finish();
    let wide = stream.current().clone();
    assert_grid(&wide, 75);
    for width in [28, 80, 38, 80] {
        stream.set_width(Some(width));
        let text = stream.current();
        assert_grid(text, usize::from(width.min(75)));
        assert!(text.lines[0].to_string().starts_with("┌────────┬"));
        assert_eq!(
            text.lines
                .iter()
                .filter(|line| line.to_string().starts_with('├'))
                .count(),
            3
        );
        assert_eq!(cell_contents(text), expected);
        assert_eq!(stream.source(), source);
        assert_eq!(text, &render(&source, width));
        assert_eq!(stream.resource_usage().table_fallbacks.width, 0);
        if width == 80 {
            assert_eq!(text, &wide);
        } else {
            assert!(text.lines.len() > wide.lines.len());
        }
    }
}

#[derive(Clone)]
struct CellStyles;

impl StyleSheet for CellStyles {
    fn code(&self) -> Style {
        Style::new().fg(Color::Blue)
    }

    fn table_header(&self) -> Style {
        Style::new().bg(Color::Yellow)
    }

    fn table_cell(&self) -> Style {
        Style::new().bg(Color::Green)
    }
}

#[test]
fn unicode_minima_keep_graphemes_and_styles_across_inline_boundaries() {
    let source = "| A | B |\n| - | - |\n| 界界 | `👩`‍💻**e**\u{301}🇨🇳 |";
    let options = Options::new(CellStyles).width(Some(11));
    let text = from_str_with_options(source, &options);
    assert_grid(&text, 11);
    assert_eq!(
        text.to_string(),
        "┌────┬────┐\n│ A  │ B  │\n├────┼────┤\n│ 界 │ 👩‍💻 │\n│ 界 │ e\u{301}  │\n│    │ 🇨🇳 │\n└────┴────┘"
    );
    let spans = text.lines.iter().flat_map(|line| &line.spans);
    let joined = spans.clone().find(|span| span.content == "👩‍💻").unwrap();
    assert_eq!(joined.style.fg, Some(Color::Blue));
    assert_eq!(joined.style.bg, Some(Color::Green));
    let accent = spans
        .clone()
        .find(|span| span.content == "e\u{301}")
        .unwrap();
    assert!(accent.style.add_modifier.contains(Modifier::BOLD));
    assert_eq!(accent.style.bg, Some(Color::Green));

    let mut stream = StreamingMarkdown::new(options.width(Some(10)));
    stream.append(source);
    assert_eq!(stream.resource_usage().table_fallbacks.width, 1);
    assert!(!stream.current().to_string().contains('┌'));
    assert!(stream.current().to_string().contains("👩‍💻"));
    assert!(stream.current().lines.iter().all(|line| line.width() <= 10));
}

#[test]
fn ascii_minimum_geometry_and_empty_cells_keep_a_grid() {
    let source = "| AB | CD |\n| - | - |\n| x | |";
    let text = render(source, 9);
    assert_grid(&text, 9);
    assert_eq!(
        text.to_string(),
        "┌───┬───┐\n│ A │ C │\n│ B │ D │\n├───┼───┤\n│ x │   │\n└───┴───┘"
    );
    let mut stream = StreamingMarkdown::new(Options::default().width(Some(8)));
    stream.append(source);
    assert_eq!(stream.resource_usage().table_fallbacks.width, 1);
}

#[test]
fn cell_padding_uses_rendered_width_for_sequences_with_contextual_width() {
    let source = "| Name | Value |\n| - | - |\n| لا | x |";
    for width in [12, 16, 80] {
        let text = render(source, width);
        assert_grid(&text, usize::from(width.min(16)));
        assert!(text.to_string().contains("لا"));
    }
}

#[test]
fn wrapping_aligns_each_physical_cell_row_and_pads_ragged_rows() {
    let source = "| Left | Middle | Right |\n| :- | :-: | -: |\n| xy | ab cd ef | xyz |\n| z |";
    let text = render(source, 22);
    assert_grid(&text, 22);
    assert_eq!(
        text.to_string(),
        [
            "┌──────┬──────┬──────┐",
            "│ Left │ Midd │ Righ │",
            "│      │  le  │    t │",
            "├──────┼──────┼──────┤",
            "│ xy   │ ab   │  xyz │",
            "│      │ cd   │      │",
            "│      │  ef  │      │",
            "├──────┼──────┼──────┤",
            "│ z    │      │      │",
            "└──────┴──────┴──────┘",
        ]
        .join("\n")
    );
}

#[test]
fn every_prefix_and_resize_matches_batch_and_retains_source() {
    let source = format!("{TABLE}\n| z | `👩`‍💻**e**\u{301}界 |\n\nAfter.\n");
    let mut stream = StreamingMarkdown::new(Options::default().width(Some(18)));
    for (start, ch) in source.char_indices() {
        let end = start + ch.len_utf8();
        stream.append(&source[start..end]);
        for width in [Some(11), Some(8), Some(18), Some(80), None, Some(18)] {
            stream.set_width(width);
            assert_eq!(stream.source(), &source[..end]);
            assert_eq!(
                stream.current(),
                &from_str_with_options(&source[..end], &Options::default().width(width)),
                "prefix={end}, width={width:?}"
            );
        }
    }
    stream.finish();
    for width in [80, 11, 18, 80] {
        stream.set_width(Some(width));
        assert_eq!(stream.source(), source);
        assert_eq!(stream.current(), &render(&source, width));
    }
    assert_eq!(stream.resource_usage().table_fallbacks.width, 0);
}

#[test]
fn default_and_custom_table_limits_still_use_stacked_fallback() {
    let source = "| A | B |\n| - | - |\n| x |";
    for (limits, expected) in [
        (
            TableLimits {
                max_cells: 3,
                ..TableLimits::default()
            },
            (1, 0),
        ),
        (
            TableLimits {
                max_buffer_bytes: 1,
                ..TableLimits::default()
            },
            (0, 1),
        ),
    ] {
        let mut stream =
            StreamingMarkdown::new(Options::default().width(Some(9)).table_limits(limits));
        stream.append(source);
        let fallbacks = stream.resource_usage().table_fallbacks;
        assert_eq!((fallbacks.cell_limit, fallbacks.buffer_limit), expected);
        assert!(stream.current().to_string().starts_with("Header"));
        assert!(stream.current().to_string().ends_with("[2] "));
    }
    assert_eq!(TableLimits::default().max_cells, 8_192);
    assert_eq!(TableLimits::default().max_buffer_bytes, 4 * 1024 * 1024);
}

fn cell_contents(text: &Text<'_>) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut cells = Vec::<String>::new();
    for line in &text.lines {
        if line.to_string().starts_with('│') {
            for (column, spans) in line.spans[1..]
                .split(|span| span.content == "│")
                .filter(|spans| !spans.is_empty())
                .enumerate()
            {
                cells.resize_with(cells.len().max(column + 1), String::new);
                // Padding is separate from content, including spaces preserved by the wrapper.
                for span in spans.iter().skip(1).take(spans.len().saturating_sub(2)) {
                    cells[column].push_str(&span.content);
                }
            }
        } else if !cells.is_empty() {
            rows.push(std::mem::take(&mut cells));
        }
    }
    rows
}

#[test]
fn many_widths_preserve_cell_text_and_style_runs_without_replacing_wide_graphemes() {
    let source = "| ID | Value | S |\n| - | :-: | -: |\n\
                  | x | aa **bb**cc  dd | z |\n\
                  | y | abcdefghijklmno | |\n\
                  | z | 界`👩`‍💻**e**\u{301}🇨🇳 ok | q |\n\
                  | n | |";
    for width in 15..=64 {
        let text = render(source, width);
        assert!(text.to_string().starts_with('┌'), "width={width}: {text}");
        let actual_width = text.lines[0].width();
        assert!(actual_width <= usize::from(width));
        assert_grid(&text, actual_width);
        assert_eq!(
            cell_contents(&text),
            [
                vec!["ID", "Value", "S"],
                vec!["x", "aa bbcc  dd", "z"],
                vec!["y", "abcdefghijklmno", ""],
                vec!["z", "界👩‍💻e\u{301}🇨🇳 ok", "q"],
                vec!["n", "", ""],
            ],
            "width={width}"
        );
        let bold = text
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .filter(|span| span.style.add_modifier.contains(Modifier::BOLD))
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(bold.ends_with("bbe\u{301}"), "width={width}: {bold:?}");
    }
}

#[test]
fn nested_lists_and_quotes_reserve_body_width_for_the_whole_grid() {
    for (preamble, first_prefix, source_continuation, display_continuation, width) in [
        ("", "- ", "  ", "  ", 20),
        ("", "10. ", "    ", "    ", 22),
        ("- Parent\n", "  - ", "    ", "      ", 24),
        ("", "> ", "> ", "> ", 20),
        ("", "> > ", "> > ", ">> ", 21),
        ("", "> - ", ">   ", ">   ", 22),
        ("", "- > ", "  > ", ">   ", 22),
        ("", "> - > ", ">   > ", ">>   ", 23),
        ("- Intro\n\n", "  ", "  ", "  ", 20),
    ] {
        let mut source = preamble.to_owned();
        for (index, line) in TABLE.lines().enumerate() {
            source.push_str(if index == 0 {
                first_prefix
            } else {
                source_continuation
            });
            source.push_str(line);
            source.push('\n');
        }
        let text = render(&source, width);
        assert!(
            text.lines
                .iter()
                .all(|line| line.width() <= usize::from(width)),
            "{text}"
        );
        let top = text
            .lines
            .iter()
            .position(|line| line.to_string().contains('┌'))
            .unwrap();
        let grid = render(TABLE, 18);
        for (index, line) in grid.lines.iter().enumerate() {
            let actual = text.lines[top + index].to_string();
            if index == 0 {
                assert!(actual.ends_with(&line.to_string()), "{text}");
                assert_eq!(text.lines[top].width(), usize::from(width), "{text}");
            } else {
                assert_eq!(
                    actual,
                    format!("{display_continuation}{line}"),
                    "{source}\n{text}"
                );
            }
        }
    }
}

#[test]
fn confirmed_wrapped_table_is_reused_for_suffix_appends_and_cached_reads() {
    let source = format!("{TABLE}\n\nTail");
    let options = Options::default().width(Some(18));
    let mut stream = StreamingMarkdown::new(options.clone());
    stream.append(&source);
    let prefix = stream.current().lines[..10].to_vec();
    let before = stream.counters();
    let pointer = stream.current().lines.as_ptr();
    for _ in 0..10 {
        stream.set_width(Some(18));
        assert_eq!(stream.prepare_rows(0, 10).rows(), prefix);
        assert_eq!(stream.current().lines.as_ptr(), pointer);
    }
    assert_eq!(stream.counters(), before);
    let update = stream.append(" suffix");
    let delta = stream.counters() - before;
    assert!(update.replay_start >= TABLE.len(), "{update:?}");
    assert!(update.stable_rows >= prefix.len(), "{update:?}");
    assert!(delta.processed_source_bytes < 32, "{delta:?}");
    assert_eq!(stream.current().lines[..10], prefix);
    assert_eq!(
        stream.current(),
        &from_str_with_options(&format!("{source} suffix"), &options)
    );
    stream.finish();
    stream.append(" reopened");
    assert_eq!(stream.source(), format!("{source} suffix reopened"));
    assert_eq!(
        stream.current(),
        &from_str_with_options(stream.source(), &options)
    );
}

#[test]
fn nested_grid_prefixes_and_resizes_match_batch_and_restore_legacy_placement() {
    let source = "- > | A | B |\n  > | - | - |\n  > | long words | 👩‍💻e\u{301} |\n";
    let mut stream = StreamingMarkdown::new(Options::default().width(Some(18)));
    for (start, ch) in source.char_indices() {
        let end = start + ch.len_utf8();
        stream.append(&source[start..end]);
        for width in [Some(18), Some(11), Some(80), None] {
            stream.set_width(width);
            assert_eq!(stream.source(), &source[..end]);
            assert_eq!(
                stream.current(),
                &from_str_with_options(&source[..end], &Options::default().width(width)),
                "prefix={end}, width={width:?}"
            );
        }
    }
    assert!(stream.current().lines[0].to_string().starts_with("- ┌"));
    stream.set_width(Some(18));
    assert!(stream.current().lines[0].to_string().starts_with("> - ┌"));
    stream.set_width(None);
    assert!(stream.current().lines[0].to_string().starts_with("- ┌"));
}
