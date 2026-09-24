use ratatui_core::style::{Color, Modifier, Style};
use tui_markdown::{from_str_with_options, Options, StyleSheet, TableLimits};

#[derive(Clone)]
struct PaneStyles;

impl StyleSheet for PaneStyles {
    fn heading_marker(&self, _: u8) -> &str {
        ""
    }

    fn code_block_fence(&self) -> &str {
        ""
    }

    fn heading(&self, _: u8) -> Style {
        Style::new().fg(Color::Reset).add_modifier(Modifier::BOLD)
    }

    fn code(&self) -> Style {
        Style::new().fg(Color::Reset)
    }
}

#[test]
fn width_rows_wrap_without_losing_spaces_or_styles() {
    let text = from_str_with_options("ab **cdef** gh", &Options::new(PaneStyles).width(Some(4)));
    assert_eq!(text.to_string(), "ab \ncdef\n gh");
    assert!(text.lines.iter().all(|line| line.width() <= 4));
    assert!(text.lines[1]
        .spans
        .first()
        .unwrap()
        .style
        .add_modifier
        .contains(Modifier::BOLD));
}

#[test]
fn width_rows_keep_graphemes_whole() {
    let input = "ab\u{754c}e\u{301}\u{1f469}\u{200d}\u{1f4bb}z";
    let text = from_str_with_options(input, &Options::new(PaneStyles).width(Some(3)));
    assert_eq!(
        text.to_string(),
        "ab\n\u{754c}e\u{301}\n\u{1f469}\u{200d}\u{1f4bb}z"
    );
    assert!(text.lines.iter().all(|line| line.width() <= 3));
    assert_eq!(
        text.lines
            .iter()
            .map(ToString::to_string)
            .collect::<String>(),
        input
    );
}

#[test]
fn width_one_replaces_only_overwide_graphemes_with_a_hyphen() {
    let input = "A\u{754c}e\u{301}\u{1f469}\u{1f3fd}\u{200d}\u{1f4bb}\u{1f1e8}\u{1f1f3}Z";
    let options = Options::new(PaneStyles).width(Some(1));
    let text = from_str_with_options(input, &options);

    assert_eq!(text.to_string(), "A\n-\ne\u{301}\n-\n-\nZ");
    assert!(text.lines.iter().all(|line| line.width() <= 1));
    let wider = from_str_with_options(input, &Options::new(PaneStyles).width(Some(80)));
    assert_eq!(
        wider.to_string(),
        input,
        "narrow layout must not modify the source"
    );
}

#[test]
fn width_zero_has_no_display_rows() {
    let options = Options::new(PaneStyles).width(Some(0));
    let text = from_str_with_options("# Heading\n\n\u{754c}", &options);
    assert!(text.lines.is_empty());
}

#[test]
fn width_shortage_at_line_end_wraps_instead_of_replacing() {
    let text = from_str_with_options("ab\u{754c}", &Options::new(PaneStyles).width(Some(3)));
    assert_eq!(text.to_string(), "ab\n\u{754c}");
}

#[test]
fn width_rows_move_a_fitting_token_instead_of_splitting_it() {
    let prefix = "1234567890123456789012345678901";
    let input = format!("{prefix} WRAPPED_WORD_END");
    let text = from_str_with_options(&input, &Options::new(PaneStyles).width(Some(36)));

    assert_eq!(text.to_string(), format!("{prefix} \nWRAPPED_WORD_END"));
    assert!(text.lines.iter().all(|line| line.width() <= 36));
    assert_eq!(
        text.lines
            .iter()
            .map(ToString::to_string)
            .collect::<String>(),
        input
    );
}

#[test]
fn width_rows_keep_a_fitting_ascii_token_after_cjk_punctuation_contiguous() {
    let prefix = format!("{}；", "最".repeat(21));
    let input = format!("{prefix}LONG_ASCII_SUFFIX");
    let text = from_str_with_options(&input, &Options::new(PaneStyles).width(Some(52)));

    assert_eq!(text.to_string(), format!("{prefix}\nLONG_ASCII_SUFFIX"));
    assert!(text.lines.iter().all(|line| line.width() <= 52));
    assert_eq!(
        text.lines
            .iter()
            .map(ToString::to_string)
            .collect::<String>(),
        input
    );
}

#[test]
fn width_replacement_is_configurable_and_preserves_style() {
    let options = Options::new(PaneStyles)
        .width(Some(1))
        .with_wide_grapheme_replacement('*')
        .unwrap();
    let text = from_str_with_options("**\u{754c}**", &options);
    assert_eq!(text.to_string(), "*");
    assert!(text.lines[0].spans[0]
        .style
        .add_modifier
        .contains(Modifier::BOLD));
}

#[test]
fn width_replacement_accepts_only_printable_ascii() {
    let options = Options::default().width(Some(1));
    for byte in 0..=127u8 {
        let result = options
            .clone()
            .with_wide_grapheme_replacement(char::from(byte));
        assert_eq!(
            result.is_ok(),
            (32..=126).contains(&byte),
            "ASCII byte {byte}"
        );
    }
    for character in ['\u{00a0}', '\u{00e9}', '\u{754c}', '\u{1f600}', '\u{200d}'] {
        let error = options
            .clone()
            .with_wide_grapheme_replacement(character)
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "wide-grapheme replacement must be a printable ASCII character"
        );
    }
    assert_eq!(from_str_with_options("\u{754c}", &options).to_string(), "-");
}

#[test]
fn width_replacement_does_not_modify_unwrapped_batch_behavior() {
    let options = Options::new(PaneStyles);
    let input = "\u{754c}";
    let narrow_options = options
        .clone()
        .width(Some(1))
        .with_wide_grapheme_replacement('!')
        .unwrap();
    assert_eq!(
        from_str_with_options(input, &narrow_options).to_string(),
        "!"
    );
    assert_eq!(from_str_with_options(input, &options).to_string(), input);
}

#[test]
fn width_rows_join_graphemes_across_inline_style_boundaries() {
    let input = "ab`\u{1f469}`\u{200d}\u{1f4bb}z";
    let text = from_str_with_options(input, &Options::new(PaneStyles).width(Some(80)));
    assert_eq!(text.to_string(), "ab\u{1f469}\u{200d}\u{1f4bb}z");
    let joined = text.lines[0]
        .spans
        .iter()
        .find(|span| span.content.contains("\u{1f469}\u{200d}\u{1f4bb}"));
    assert!(
        joined.is_some(),
        "Ratatui must receive the complete grapheme in one span"
    );
    assert_eq!(joined.unwrap().style.fg, Some(Color::Reset));
}

#[test]
fn width_rows_preserve_wide_batch_output_and_hidden_markers() {
    let options = Options::new(PaneStyles);
    let input = "# Heading\n\n```unknown-language\ncode\n```\n\nAfter";
    let text = from_str_with_options(input, &options.clone().width(Some(80)));
    assert_eq!(text, from_str_with_options(input, &options));
    assert!(!text.to_string().contains('#'));
    assert!(!text.to_string().contains("```"));
}

#[test]
fn width_tables_wrap_cells_when_natural_grid_does_not_fit() {
    let input = "| A | B |\n| - | - |\n| alpha | beta |";
    let text = from_str_with_options(input, &Options::new(PaneStyles).width(Some(12)));
    assert_eq!(
        text.to_string(),
        "┌─────┬────┐\n│ A   │ B  │\n├─────┼────┤\n│ alp │ be │\n│ ha  │ ta │\n└─────┴────┘"
    );
    assert!(text.lines.iter().all(|line| line.width() <= 12));
}

#[test]
fn width_tables_cell_limit_preserves_every_row_and_cell() {
    let input = "| A | B |\n| - | - |\n| **one** | two |\n| three | four |";
    let options = Options::new(PaneStyles)
        .width(Some(80))
        .table_limits(TableLimits {
            max_cells: 3,
            max_buffer_bytes: 4 * 1024 * 1024,
        });
    let text = from_str_with_options(input, &options);
    assert_eq!(
        text.to_string(),
        "Header\n[1] A\n[2] B\n\nRow 1\n[1] one\n[2] two\n\nRow 2\n[1] three\n[2] four"
    );
    assert!(text.lines[5]
        .spans
        .last()
        .unwrap()
        .style
        .add_modifier
        .contains(Modifier::BOLD));
}

#[test]
fn width_tables_byte_limit_and_surrounding_blocks() {
    let input = "Before\n\n| A | B |\n| - | - |\n| one | two |\n\nAfter";
    let options = Options::new(PaneStyles)
        .width(Some(80))
        .table_limits(TableLimits {
            max_cells: usize::MAX,
            max_buffer_bytes: 1,
        });
    let text = from_str_with_options(input, &options);
    assert_eq!(
        text.to_string(),
        "Before\n\nHeader\n[1] A\n[2] B\n\nRow 1\n[1] one\n[2] two\n\nAfter"
    );
}

#[test]
fn width_tables_wide_grid_matches_existing_batch() {
    let input = "| A | B |\n| :- | -: |\n| one | two |";
    let options = Options::new(PaneStyles);
    let text = from_str_with_options(input, &options.clone().width(Some(80)));
    assert_eq!(text, from_str_with_options(input, &options));
}

#[test]
fn width_tables_count_padded_empty_cells_against_the_cell_budget() {
    let input = "| A | B |\n| - | - |\n| x |";
    let options = Options::new(PaneStyles);
    let stacked_options = options.clone().width(Some(80)).table_limits(TableLimits {
        max_cells: 3,
        max_buffer_bytes: 4 * 1024 * 1024,
    });
    let mut stacked = tui_markdown::StreamingMarkdown::new(stacked_options);
    stacked.append(input);
    assert_eq!(stacked.resource_usage().table_fallbacks.cell_limit, 1);
    assert_eq!(
        stacked.current().to_string(),
        "Header\n[1] A\n[2] B\n\nRow 1\n[1] x\n[2] "
    );

    let exact_options = options.width(Some(80)).table_limits(TableLimits {
        max_cells: 4,
        max_buffer_bytes: 4 * 1024 * 1024,
    });
    let mut exact = tui_markdown::StreamingMarkdown::new(exact_options);
    exact.append(input);
    assert_eq!(exact.resource_usage().table_fallbacks.cell_limit, 0);
    assert!(exact.current().to_string().starts_with('┌'));
}

#[test]
fn width_tables_measure_graphemes_across_inline_style_spans() {
    let input = "| `\u{1f469}`\u{200d}\u{1f4bb} |\n| - |";
    let options = Options::new(PaneStyles);
    let grid_options = options.clone().width(Some(6));
    let mut grid = tui_markdown::StreamingMarkdown::new(grid_options.clone());
    grid.append(input);

    assert_eq!(grid.resource_usage().table_fallbacks.width, 0);
    assert!(grid.current().lines.iter().all(|line| line.width() <= 6));
    assert_eq!(grid.current(), &from_str_with_options(input, &grid_options));
    assert!(grid
        .current()
        .to_string()
        .contains("\u{1f469}\u{200d}\u{1f4bb}"));

    let narrow_options = options.width(Some(5));
    let mut narrow = tui_markdown::StreamingMarkdown::new(narrow_options.clone());
    narrow.append(input);
    assert_eq!(narrow.resource_usage().table_fallbacks.width, 1);
    assert!(narrow.current().lines.iter().all(|line| line.width() <= 5));
    assert_eq!(
        narrow.current(),
        &from_str_with_options(input, &narrow_options)
    );
    assert!(narrow
        .current()
        .to_string()
        .contains("\u{1f469}\u{200d}\u{1f4bb}"));
}
