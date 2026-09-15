use ratatui_core::style::{Color, Modifier, Style};
use tui_markdown::{
    from_str_with_context, ChangeReason, Options, RenderContext, StreamingMarkdown, StyleSheet,
    TableLimits,
};

#[derive(Clone)]
struct AlternateStyles;

impl StyleSheet for AlternateStyles {
    fn heading(&self, _: u8) -> Style {
        Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    }
}

fn assert_matches_batch(stream: &StreamingMarkdown, source: &str, width: u16) {
    assert_eq!(stream.source(), source);
    assert_eq!(
        stream.current(),
        &from_str_with_context(source, &Options::default(), &RenderContext::new(width)),
        "source={source:?}"
    );
}

#[test]
fn append_reuses_completed_blocks_and_reports_work() {
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(40));
    let source = "first\n\nsecond\n\nthird";
    stream.append(source);
    let before = stream.counters();

    let update = stream.append("\n\nfourth");
    let delta = stream.counters() - before;

    assert_matches_batch(&stream, &format!("{source}\n\nfourth"), 40);
    assert_eq!(update.reason, ChangeReason::Append);
    assert!(
        update.replay_start >= "first\n\nsecond\n\n".len(),
        "{update:?}"
    );
    assert!(update.stable_rows >= 2);
    assert!(delta.processed_source_bytes < source.len() as u64);
    assert!(delta.parsed_events > 0);
    assert!(delta.recomputed_blocks <= 2);
}

#[test]
fn adjacent_self_terminating_blocks_do_not_pin_replay_to_document_start() {
    let source = (0..200)
        .map(|index| format!("# heading {index}\n"))
        .collect::<String>();
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(40));
    stream.append(&source);
    let before = stream.counters();

    let update = stream.append("# tail\n");
    let delta = stream.counters() - before;

    assert_matches_batch(&stream, &format!("{source}# tail\n"), 40);
    assert!(update.replay_start > source.len() - 32, "{update:?}");
    assert!(delta.processed_source_bytes < 64, "{delta:?}");
}

#[test]
fn prepared_viewport_rows_borrow_the_cached_display_without_work() {
    let source = (0..200)
        .map(|index| format!("row {index}\n\n"))
        .collect::<String>();
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(40));
    stream.append(&source);
    let before = stream.counters();

    let viewport = stream.prepare_rows(75, 12);

    assert_eq!(viewport.first_row(), 75);
    assert_eq!(viewport.total_rows(), stream.current().lines.len());
    assert_eq!(viewport.rows().len(), 12);
    assert!(std::ptr::eq(
        viewport.rows().as_ptr(),
        stream.current().lines[75..].as_ptr()
    ));
    assert_eq!(stream.counters(), before);

    let repeated = stream.prepare_rows(75, 12);
    assert!(std::ptr::eq(
        viewport.rows().as_ptr(),
        repeated.rows().as_ptr()
    ));
    assert_eq!(stream.counters(), before);
}

#[test]
fn prepared_viewport_rows_clamp_out_of_range_requests() {
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(20));
    stream.append("one\n\ntwo");
    let total = stream.current().lines.len();

    let tail = stream.prepare_rows(total.saturating_sub(1), usize::MAX);
    assert_eq!(tail.rows().len(), usize::from(total != 0));
    let outside = stream.prepare_rows(usize::MAX, 50);
    assert_eq!(outside.first_row(), total);
    assert_eq!(outside.total_rows(), total);
    assert!(outside.rows().is_empty());
}

#[test]
fn no_op_operations_do_zero_work_and_finish_once_per_version() {
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(40));
    stream.append("hello **world**");

    let before = stream.counters();
    assert_eq!(stream.append("").reason, ChangeReason::None);
    assert_eq!(stream.counters(), before);
    let _ = stream.current();
    let _ = stream.current();
    assert_eq!(stream.counters(), before);

    let first = stream.finish();
    let after_finish = stream.counters();
    assert_eq!(first.reason, ChangeReason::Finish);
    assert_eq!(
        after_finish.full_recomputations,
        before.full_recomputations + 1
    );
    assert_eq!(stream.finish().reason, ChangeReason::None);
    assert_eq!(stream.counters(), after_finish);

    assert_eq!(stream.append("!").reason, ChangeReason::Reopen);
    assert_eq!(stream.finish().reason, ChangeReason::Finish);
    assert_eq!(
        stream.counters().full_recomputations,
        after_finish.full_recomputations + 2
    );
}

#[test]
fn replace_clear_context_and_reuse_have_canonical_snapshots() {
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(8));
    stream.append("long paragraph");
    stream.replace("same length!!!");
    assert_matches_batch(&stream, "same length!!!", 8);

    stream.replace("tiny");
    assert_matches_batch(&stream, "tiny", 8);

    let before_context = stream.counters();
    let update = stream.set_context(RenderContext::new(1));
    assert_eq!(update.reason, ChangeReason::Context);
    assert_matches_batch(&stream, "tiny", 1);
    assert_eq!(
        stream.counters().context_reflows,
        before_context.context_reflows + 1
    );
    assert_eq!(
        stream.counters().processed_source_bytes,
        before_context.processed_source_bytes
    );
    assert_eq!(
        stream.counters().parsed_events,
        before_context.parsed_events
    );

    assert_eq!(stream.clear().reason, ChangeReason::Clear);
    assert_matches_batch(&stream, "", 1);
    stream.append("\u{754c}");
    assert_matches_batch(&stream, "\u{754c}", 1);
    stream.finish();
    stream.clear();
    stream.append("reused");
    assert_matches_batch(&stream, "reused", 1);
}

#[test]
fn references_and_footnotes_trigger_explicit_global_recomputation() {
    for (first, second) in [
        ("A [link][target].", "\n\n[target]: https://example.test"),
        ("A note[^1].", "\n\n[^1]: Later."),
    ] {
        let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(80));
        stream.append(first);
        let before = stream.counters();
        let update = stream.append(second);
        let delta = stream.counters() - before;

        assert_matches_batch(&stream, &format!("{first}{second}"), 80);
        assert_eq!(update.reason, ChangeReason::GlobalDependency);
        assert_eq!(update.replay_start, 0);
        assert_eq!(update.stable_rows, 0);
        assert!(delta.full_recomputations >= 1, "{update:?} {delta:?}");
    }
}

#[test]
fn discovering_a_global_dependency_does_not_revoke_confirmed_rows() {
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(80));
    let before_global = stream.append("confirmed\n\nmutable");
    assert!(before_global.stable_rows > 0);

    let unresolved = stream.append("\n\nA [reference][later].");
    assert_eq!(unresolved.reason, ChangeReason::GlobalDependency);
    assert!(unresolved.stable_rows >= before_global.stable_rows);

    let definition = stream.append("\n\n[later]: https://example.test");
    assert_eq!(definition.reason, ChangeReason::GlobalDependency);
    assert!(definition.stable_rows >= before_global.stable_rows);
    assert_matches_batch(
        &stream,
        "confirmed\n\nmutable\n\nA [reference][later].\n\n[later]: https://example.test",
        80,
    );
}

#[test]
fn arbitrary_utf8_chunks_match_every_batch_prefix() {
    let documents = [
        "# Heading\n\nParagraph with **bold**, `code`, and e\u{301}.",
        "> quote\n>\n> - one\n> - two\n\nAfter",
        "```rust\nfn main() {\n    println!(\"hi\");\n}\n```",
        "| A | B |\n| :- | -: |\n| one | \u{754c} |\n| wider | value |",
        "A [link][id].\r\n\r\n[id]: https://example.test\r\n",
        "Footnote[^a]\n\n[^a]: first\n\n    second",
    ];

    for (document_index, document) in documents.iter().enumerate() {
        for seed in 0..12u64 {
            let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(17));
            let boundaries: Vec<_> = document
                .char_indices()
                .map(|(index, _)| index)
                .chain(std::iter::once(document.len()))
                .collect();
            let mut cursor = 0;
            let mut state = seed + document_index as u64 * 17 + 1;
            while cursor < boundaries.len() - 1 {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                let step = (state as usize % 7) + 1;
                let next = (cursor + step).min(boundaries.len() - 1);
                stream.append(&document[boundaries[cursor]..boundaries[next]]);
                assert_matches_batch(&stream, &document[..boundaries[next]], 17);
                cursor = next;
            }
            stream.finish();
            assert_matches_batch(&stream, document, 17);
        }
    }
}

#[test]
fn every_utf8_prefix_matches_for_incomplete_and_reclassified_syntax() {
    let corpus = [
        "plain\nsoft  \nhard\\\nnext\n\nparagraph",
        "# split heading {#id}\n\nSetext\n===\n\n---",
        "- tight\n  - nested\n- list\n\n  loose\n\n10. ordered",
        "> [!NOTE]\n> quoted\n>\n> continuation\n\noutside",
        "````rust\n```\ninside\n```\n````\n\n    indented",
        "| h\\|x | `a|b` |\n| :--- | ---: |\n| one | two |\nnext",
        "[inline](destination \"title\") and ![alt](image.png)",
        "[forward][ref]\n\n[ref]: https://example.test \"title\"",
        "note[^label]\n\n[^label]: first\n\n    second",
        "~~strike~~ H~2~O x^2^ $math$ $$display$$",
        "---\r\nkey: value\r\n---\r\n\r\nAfter\r\n",
        "\u{1f469}\u{1f3fd}\u{200d}\u{1f4bb} e\u{301} \u{1f1e8}\u{1f1f3}",
    ];

    for document in corpus {
        let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(23));
        let mut previous = 0;
        for next in document
            .char_indices()
            .map(|(index, _)| index)
            .skip(1)
            .chain(std::iter::once(document.len()))
        {
            stream.append(&document[previous..next]);
            assert_matches_batch(&stream, &document[..next], 23);
            previous = next;
        }
    }
}

#[test]
fn interleaved_documents_keep_independent_source_and_projection_state() {
    let mut left = StreamingMarkdown::new(Options::default(), RenderContext::new(7));
    let mut right = StreamingMarkdown::new(Options::default(), RenderContext::new(13));

    left.append("| A |\n| - |");
    right.append("**right");
    left.append("\n| wide value |");
    right.append(" side**");

    assert_matches_batch(&left, "| A |\n| - |\n| wide value |", 7);
    assert_matches_batch(&right, "**right side**", 13);
    left.clear();
    right.append("\n\nstill right");
    assert_matches_batch(&left, "", 7);
    assert_matches_batch(&right, "**right side**\n\nstill right", 13);
}

#[test]
fn replacing_options_restyles_the_canonical_snapshot() {
    let context = RenderContext::new(80);
    let mut stream = StreamingMarkdown::new(Options::new(AlternateStyles), context);
    stream.append("# heading");
    let options =
        Options::new(AlternateStyles).image_fallback(tui_markdown::ImageFallback::AltTextAndUrl);

    let update = stream.set_options(options.clone());

    assert_eq!(update.reason, ChangeReason::Options);
    assert_eq!(
        stream.current(),
        &from_str_with_context("# heading", &options, &context)
    );
    assert_eq!(stream.source(), "# heading");
}

#[test]
fn resource_usage_accounts_for_source_and_owned_snapshots() {
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(20));
    stream.append("owned **content**");
    let usage = stream.resource_usage();

    assert_eq!(usage.source_bytes, stream.source().len());
    assert!(usage.source_capacity_bytes >= usage.source_bytes);
    assert!(usage.current_bytes >= "owned content".len());
    assert!(usage.current_capacity_bytes >= usage.current_bytes);
    assert!(usage.semantic_capacity_bytes >= usage.current_bytes);
    assert!(usage.checkpoint_count <= 1);
}

#[test]
fn resource_usage_reports_each_table_fallback_reason() {
    let defaults = RenderContext::new(80).limits();
    assert_eq!(defaults.max_cells, 8_192);
    assert_eq!(defaults.max_buffer_bytes, 4 * 1024 * 1024);

    let input = "| A | B |\n| - | - |\n| one | two |";
    let cases = [
        (RenderContext::new(5), (1, 0, 0)),
        (
            RenderContext::new(80).table_limits(TableLimits {
                max_cells: 3,
                max_buffer_bytes: 4 * 1024 * 1024,
            }),
            (0, 1, 0),
        ),
        (
            RenderContext::new(80).table_limits(TableLimits {
                max_cells: usize::MAX,
                max_buffer_bytes: 1,
            }),
            (0, 0, 1),
        ),
    ];
    for (context, expected) in cases {
        let mut stream = StreamingMarkdown::new(Options::default(), context);
        stream.append(input);
        assert_eq!(
            stream.current(),
            &from_str_with_context(input, &Options::default(), &context)
        );
        let fallbacks = stream.resource_usage().table_fallbacks;
        assert_eq!(
            (
                fallbacks.width,
                fallbacks.cell_limit,
                fallbacks.buffer_limit
            ),
            expected
        );
    }
}

#[test]
fn default_table_cell_budget_switches_to_stacked_without_losing_tail_content() {
    let mut input = String::from("| A | B |\n| - | - |\n");
    for _ in 0..4_095 {
        input.push_str("| x | y |\n");
    }
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(80));
    stream.append(&input);
    assert_eq!(stream.resource_usage().table_fallbacks.cell_limit, 0);

    stream.append("| final-left |");

    let usage = stream.resource_usage();
    assert_eq!(usage.table_fallbacks.cell_limit, 1);
    let rendered = stream.current().to_string();
    assert!(rendered.contains("Row 4096"));
    assert!(rendered.contains("[1] final-left"));
    assert!(rendered.contains("[2] "));
}

#[test]
fn table_context_changes_recompute_canonical_geometry_and_fallbacks() {
    let input = "| A | B |\n| - | - |\n| long value | two |";
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(80));
    stream.append(input);
    let before = stream.counters();

    let narrow = RenderContext::new(9);
    let update = stream.set_context(narrow);

    assert_eq!(update.reason, ChangeReason::Context);
    assert_eq!(
        stream.current(),
        &from_str_with_context(input, &Options::default(), &narrow)
    );
    assert!(stream.counters().parsed_events > before.parsed_events);
    assert_eq!(stream.resource_usage().table_fallbacks.width, 1);
}

#[test]
fn trace_diagnostics_do_not_contain_markdown_source() {
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};

    use tracing::level_filters::LevelFilter;
    use tracing::subscriber;
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::fmt::MakeWriter;

    #[derive(Clone)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for CapturedWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for Captured {
        type Writer = CapturedWriter;

        fn make_writer(&'a self) -> Self::Writer {
            CapturedWriter(Arc::clone(&self.0))
        }
    }

    let output = Arc::new(Mutex::new(Vec::new()));
    let tracing = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(Captured(Arc::clone(&output)))
        .with_max_level(LevelFilter::TRACE)
        .with_span_events(FmtSpan::NEW | FmtSpan::ENTER)
        .finish();
    let _guard = subscriber::set_default(tracing);
    let secret = "sensitive-markdown-9f36";
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(80));
    stream.append(&format!(
        "[{secret}](https://{secret}.invalid)\n\n```{secret}\n{secret}\n```"
    ));
    drop(_guard);

    let diagnostics = String::from_utf8(output.lock().unwrap().clone()).unwrap();
    assert!(!diagnostics.contains(secret), "{diagnostics}");
}

#[test]
fn append_after_finish_explicitly_reopens_a_new_lineage() {
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(80));
    stream.append("hello");
    let finished = stream.finish();
    assert_eq!(finished.stable_rows, stream.current().lines.len());
    let before = stream.counters();

    let reopened = stream.append(" world");

    assert_eq!(reopened.reason, ChangeReason::Reopen);
    assert_eq!(reopened.replay_start, 0);
    assert_eq!(reopened.stable_rows, 0);
    assert_matches_batch(&stream, "hello world", 80);
    assert_eq!((stream.counters() - before).full_recomputations, 1);
}

#[test]
fn late_reference_after_finish_reopens_without_false_stability() {
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(80));
    stream.append("A [link][later].");
    stream.finish();

    let reopened = stream.append("\n\n[later]: /target");

    assert_eq!(reopened.reason, ChangeReason::Reopen);
    assert_eq!(reopened.replay_start, 0);
    assert_eq!(reopened.stable_rows, 0);
    assert_matches_batch(&stream, "A [link][later].\n\n[later]: /target", 80);
    let after = stream.counters();
    assert_eq!(stream.finish().reason, ChangeReason::Finish);
    assert_eq!(
        stream.counters().full_recomputations,
        after.full_recomputations + 1
    );
    let repeated = stream.counters();
    assert_eq!(stream.finish().reason, ChangeReason::None);
    assert_eq!(stream.counters(), repeated);
}

#[test]
fn replacing_finished_source_starts_a_canonical_replacement_lineage() {
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(80));
    stream.append("finished");
    stream.finish();
    let before = stream.counters();

    let replaced = stream.replace("replacement");

    assert_eq!(replaced.reason, ChangeReason::Replace);
    assert_matches_batch(&stream, "replacement", 80);
    assert_eq!((stream.counters() - before).full_recomputations, 1);
}

#[test]
fn whitespace_only_blank_lines_advance_replay_past_a_long_table() {
    let mut table = String::from("| A | B |\n| - | - |\n");
    for index in 0..256 {
        table.push_str(&format!("| row {index} | value |\n"));
    }

    for separator in ["\n   \n", "\r\n\t \r\n"] {
        let source = format!("{table}{separator}stream");
        let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(80));
        stream.append(&source);
        let before = stream.counters();

        let update = stream.append("x");
        let delta = stream.counters() - before;

        assert_matches_batch(&stream, &format!("{source}x"), 80);
        assert!(update.replay_start >= table.len(), "{update:?}");
        assert!(delta.processed_source_bytes < 32, "{delta:?}");
    }
}

#[test]
fn full_recomputation_counter_counts_executed_full_passes_once() {
    let mut stream = StreamingMarkdown::new(Options::default(), RenderContext::new(80));
    let replacement = "A [link][id].\n\n[id]: /target";
    let before_replace = stream.counters();
    stream.replace(replacement);
    let replace_delta = stream.counters() - before_replace;
    assert_eq!(replace_delta.full_recomputations, 1);
    assert_eq!(
        replace_delta.processed_source_bytes,
        replacement.len() as u64
    );

    let before_finish = stream.counters();
    stream.finish();
    assert_eq!((stream.counters() - before_finish).full_recomputations, 1);
    let repeated = stream.counters();
    stream.finish();
    assert_eq!(stream.counters(), repeated);

    let before_global_append = stream.counters();
    stream.append("\n\nMore");
    assert_eq!(
        (stream.counters() - before_global_append).full_recomputations,
        1
    );

    let mut ordinary = StreamingMarkdown::new(Options::default(), RenderContext::new(80));
    ordinary.append("first\n\nsecond");
    let before_ordinary = ordinary.counters();
    ordinary.append("\n\nthird");
    assert_eq!(
        (ordinary.counters() - before_ordinary).full_recomputations,
        0
    );
}
