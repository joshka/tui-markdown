use ratatui_core::style::{Color, Style};
use tui_markdown::{
    from_str, from_str_with_options, ChangeReason, Options, StreamingMarkdown, StyleSheet,
    TableLimits,
};

#[test]
fn unwrapped_options_preserve_legacy_output_even_with_dormant_layout_settings() {
    let source = "Long **paragraph** \u{754c}\n\n| A | B |\n| - | - |\n| one | two |";
    let options = Options::default()
        .width(None)
        .table_limits(TableLimits {
            max_cells: 0,
            max_buffer_bytes: 0,
        })
        .with_wide_grapheme_replacement('*')
        .unwrap();
    let expected = from_str(source);
    assert_eq!(from_str_with_options(source, &options), expected);
    let mut stream = StreamingMarkdown::new(options);
    stream.append(source);
    assert_eq!(stream.current(), &expected);
    assert_eq!(stream.resource_usage().table_fallbacks, Default::default());
}

#[test]
fn one_options_value_controls_batch_and_each_streaming_prefix() {
    let corpus = [
        "# Heading\n\n**styled** text \u{754c}",
        "| A | B |\n| - | - |\n| one | two |\n\nAfter",
        "```unknown\r\none\r\n\r\ntwo\r\n```\r\n\r\nAfter",
        "> quote\n>\n> - nested\n> - list\n\noutside",
        "[forward][ref]\n\n[ref]: /target",
        "$E=mc^2$\n\n$$\nx = y\n$$",
    ];
    for width in [None, Some(0), Some(1), Some(7), Some(80)] {
        for source in corpus {
            let options = Options::default().width(width);
            let mut stream = StreamingMarkdown::new(options.clone());
            let mut start = 0;
            for end in source
                .char_indices()
                .map(|(i, _)| i)
                .skip(1)
                .chain([source.len()])
            {
                stream.append(&source[start..end]);
                assert_eq!(stream.source(), &source[..end]);
                assert_eq!(
                    stream.current(),
                    &from_str_with_options(&source[..end], &options),
                    "width={width:?}, prefix={:?}",
                    &source[..end]
                );
                start = end;
            }
            stream.finish();
            assert_eq!(stream.current(), &from_str_with_options(source, &options));
        }
    }
}

#[test]
fn identical_layout_updates_preserve_cached_rows_work_and_completion() {
    let mut stream = StreamingMarkdown::new(Options::default().width(Some(40)));
    stream.append("first\n\nsecond\n\nthird");
    stream.finish();
    let before = stream.counters();
    let rows = stream.current().lines.as_ptr();
    assert_eq!(stream.set_width(Some(40)).reason, ChangeReason::None);
    assert_eq!(
        stream.set_table_limits(TableLimits::default()).reason,
        ChangeReason::None
    );
    assert_eq!(
        stream.set_wide_grapheme_replacement('-').unwrap().reason,
        ChangeReason::None
    );
    assert_eq!(stream.finish().reason, ChangeReason::None);
    assert_eq!(stream.counters(), before);
    assert_eq!(stream.current().lines.as_ptr(), rows);
    assert_eq!(
        stream.prepare_rows(1, 2).rows().as_ptr(),
        stream.current().lines[1..].as_ptr()
    );
}

#[test]
fn width_only_updates_reflow_without_parsing_and_keep_suffix_replay() {
    let source = "# first heading\n\n# second heading\n\nlast";
    let mut stream = StreamingMarkdown::new(Options::default().width(Some(80)));
    stream.append(source);
    for width in [Some(4), Some(0), None, Some(20)] {
        let before = stream.counters();
        assert_eq!(stream.set_width(width).reason, ChangeReason::Layout);
        let after = stream.counters();
        assert_eq!(after.processed_source_bytes, before.processed_source_bytes);
        assert_eq!(after.parsed_events, before.parsed_events);
        assert_eq!(after.rendered_events, before.rendered_events);
        assert_eq!(after.full_recomputations, before.full_recomputations);
        assert_eq!(after.layout_reflows, before.layout_reflows + 1);
        assert_eq!(
            stream.current(),
            &from_str_with_options(source, &Options::default().width(width))
        );
    }
    let before = stream.counters();
    let update = stream.append(" tail");
    assert!(update.replay_start >= "# first heading\n\n".len());
    assert!((stream.counters() - before).processed_source_bytes < source.len() as u64);
    assert_eq!(
        stream.current(),
        &from_str_with_options(
            &format!("{source} tail"),
            &Options::default().width(Some(20))
        )
    );
}

#[test]
fn invalid_replacement_updates_are_atomic_and_valid_updates_do_not_reparse() {
    let mut stream = StreamingMarkdown::new(Options::default().width(Some(1)));
    stream.append("**\u{754c}**");
    stream.finish();
    let before = stream.counters();
    let rows = stream.current().lines.as_ptr();
    for replacement in ['\n', '\u{7f}', '\u{e9}', '\u{754c}'] {
        assert!(stream.set_wide_grapheme_replacement(replacement).is_err());
        assert_eq!(stream.counters(), before);
        assert_eq!(stream.current().lines.as_ptr(), rows);
        assert_eq!(stream.finish().reason, ChangeReason::None);
    }
    assert_eq!(
        stream.set_wide_grapheme_replacement('*').unwrap().reason,
        ChangeReason::Layout
    );
    assert_eq!(stream.current().to_string(), "*");
    assert_eq!(stream.counters().parsed_events, before.parsed_events);
    assert_eq!(stream.source(), "**\u{754c}**");
    stream.set_width(None);
    assert_eq!(stream.current(), &from_str("**\u{754c}**"));
}

#[test]
fn table_layout_updates_rebuild_once_and_unwrapped_output_disables_limits() {
    let source = "| A | B |\n| - | - |\n| one | two |";
    let limits = TableLimits {
        max_cells: 1,
        max_buffer_bytes: 4 * 1024 * 1024,
    };
    let mut stream = StreamingMarkdown::new(Options::default().width(Some(80)));
    stream.append(source);
    let before = stream.counters();
    let update = stream.set_table_limits(limits);
    assert_eq!(update.reason, ChangeReason::Layout);
    let delta = stream.counters() - before;
    assert_eq!(delta.full_recomputations, 1);
    assert_eq!(delta.processed_source_bytes, source.len() as u64);
    assert_eq!(stream.resource_usage().table_fallbacks.cell_limit, 1);
    assert_eq!(
        stream.current(),
        &from_str_with_options(
            source,
            &Options::default().width(Some(80)).table_limits(limits)
        )
    );
    stream.set_width(None);
    assert_eq!(stream.current(), &from_str(source));
    assert_eq!(stream.resource_usage().table_fallbacks, Default::default());
    stream.set_width(Some(80));
    assert_eq!(stream.resource_usage().table_fallbacks.cell_limit, 1);
}

#[test]
fn table_limit_updates_without_tables_do_not_parse() {
    let mut stream = StreamingMarkdown::new(Options::default().width(Some(80)));
    stream.append("plain content");
    let before = stream.counters();
    stream.set_table_limits(TableLimits {
        max_cells: 1,
        max_buffer_bytes: 1,
    });
    assert_eq!(stream.counters().parsed_events, before.parsed_events);
    assert_eq!(
        stream.counters().full_recomputations,
        before.full_recomputations
    );
    assert_eq!(stream.current().to_string(), "plain content");
}

#[derive(Clone)]
struct CustomStyle(Color);

impl StyleSheet for CustomStyle {
    fn heading(&self, _: u8) -> Style {
        Style::new().fg(self.0)
    }
}

#[test]
fn replacing_options_does_not_infer_style_equality_from_equal_layout() {
    let mut stream = StreamingMarkdown::new(Options::new(CustomStyle(Color::Blue)).width(Some(40)));
    stream.append("# Heading");
    let before = stream.counters();
    let replacement = Options::new(CustomStyle(Color::Red)).width(Some(40));
    assert_eq!(
        stream.set_options(replacement.clone()).reason,
        ChangeReason::Options
    );
    assert_eq!((stream.counters() - before).full_recomputations, 1);
    assert_eq!(
        stream.current(),
        &from_str_with_options("# Heading", &replacement)
    );
    assert_eq!(stream.current().lines[0].style.fg, Some(Color::Red));
}

#[test]
fn each_width_update_invalidates_completion_once() {
    let mut stream = StreamingMarkdown::new(Options::default().width(Some(20)));
    stream.append("content");
    stream.finish();
    stream.set_width(Some(10));
    let before = stream.counters();
    stream.finish();
    assert_eq!((stream.counters() - before).full_recomputations, 1);
    let before = stream.counters();
    stream.finish();
    assert_eq!(stream.counters(), before);
}
