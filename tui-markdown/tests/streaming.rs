use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use pulldown_cmark::Parser;
use ratatui_core::style::{Color, Style};
use ratatui_core::text::Text;
use rstest::rstest;
use tui_markdown::{
    from_str_with_options, ChangeReason, ImageFallback, Options, StreamingMarkdown, StyleSheet,
    Update, WorkCounters,
};

fn changed_row(old: &Text<'_>, new: &Text<'_>) -> Option<usize> {
    old.lines
        .iter()
        .zip(&new.lines)
        .position(|(old, new)| old != new)
        .or_else(|| {
            (old.lines.len() != new.lines.len()).then_some(old.lines.len().min(new.lines.len()))
        })
}

fn append_checked<S: StyleSheet>(
    stream: &mut StreamingMarkdown<S>,
    chunk: &str,
    options: &Options<S>,
    stable_rows: usize,
) -> Update {
    let old = stream.current().clone();
    let source = format!("{}{chunk}", stream.source());
    let expected = from_str_with_options(&source, options);
    let update = stream.append(chunk);
    assert_eq!(stream.source(), source);
    assert_eq!(stream.current(), &expected, "source={source:?}");
    assert_eq!(update.first_changed_row, changed_row(&old, &expected));
    assert!(update.stable_rows >= stable_rows, "{update:?}");
    assert_eq!(
        &old.lines[..stable_rows],
        &stream.current().lines[..stable_rows],
        "a previously stable row changed: {source:?}"
    );
    update
}

#[test]
fn source_and_snapshots_are_owned_and_documents_are_independent() {
    let options = Options::default();
    let mut left = StreamingMarkdown::new(options.clone());
    let mut right = StreamingMarkdown::new(options.clone());
    assert_eq!(left.counters(), WorkCounters::default());
    assert_eq!(left.current(), &Text::default());
    assert_eq!(left.source(), "");

    let mut chunk = String::from("left **");
    left.append(&chunk);
    chunk.clear();
    right.append("right e");
    left.append("side**");
    right.append("\u{301}\r\n\r\nnext");
    assert_eq!(left.source(), "left **side**");
    assert_eq!(right.source(), "right e\u{301}\r\n\r\nnext");
    assert_eq!(
        right.current(),
        &from_str_with_options(right.source(), &options)
    );

    let snapshot: Text<'static> = left.current().clone();
    drop(left);
    assert_eq!(snapshot.to_string(), "left side");
    assert_eq!(
        right.current(),
        &from_str_with_options(right.source(), &options)
    );
}

#[test]
fn changes_and_unchanged_reads_are_exact() {
    let options = Options::default();
    let mut stream = StreamingMarkdown::new(options.clone());
    let first = append_checked(&mut stream, "first\n\nsecond", &options, 0);
    assert_eq!(first.first_changed_row, Some(0));
    assert_eq!(first.stable_rows, 1);
    let edit = append_checked(&mut stream, "!", &options, first.stable_rows);
    assert_eq!(edit.first_changed_row, Some(2));
    let newline = append_checked(&mut stream, "\n", &options, edit.stable_rows);
    assert_eq!(newline.first_changed_row, None);

    let before = stream.counters();
    let pointer = stream.current().lines.as_ptr();
    for _ in 0..20 {
        assert_eq!(stream.current().lines.as_ptr(), pointer);
        assert_eq!(stream.source(), "first\n\nsecond!\n");
        let update = stream.append("");
        assert_eq!(update.reason, ChangeReason::None);
        assert_eq!(update.first_changed_row, None);
        assert_eq!(update.stable_rows, newline.stable_rows);
    }
    assert_eq!(stream.counters(), before);
}

#[test]
fn ordinary_append_counts_only_the_actual_suffix() {
    let mut stream = StreamingMarkdown::new(Options::default());
    let source = "first\n\nsecond\n\nthird";
    stream.append(source);
    assert_eq!(
        stream.counters().processed_source_bytes,
        source.len() as u64
    );
    assert_eq!(
        stream.counters().parsed_events,
        Parser::new(source).count() as u64
    );
    let before = stream.counters();
    let pointer = stream.current().lines[0].spans[0].content.as_ptr();

    let update = stream.append("\n\nfourth");
    let work = stream.counters() - before;
    assert_eq!(update.reason, ChangeReason::Append);
    assert_eq!(update.replay_start, "first\n\nsecond\n\n".len());
    assert_eq!(work.processed_source_bytes, "third\n\nfourth".len() as u64);
    assert_eq!(work.parsed_events, 6);
    assert_eq!(work.rendered_events, 6);
    assert_eq!(work.recomputed_blocks, 2);
    assert_eq!(work.full_recomputations, 0);
    assert_eq!(stream.current().lines[0].spans[0].content.as_ptr(), pointer);
    assert_eq!(
        stream.current(),
        &from_str_with_options(stream.source(), &Options::default())
    );
}

#[derive(Clone)]
struct CountingStyles(Arc<AtomicUsize>);

impl StyleSheet for CountingStyles {
    fn heading(&self, _: u8) -> Style {
        self.0.fetch_add(1, Ordering::Relaxed);
        Style::new().yellow()
    }
}

#[test]
fn adjacent_headings_reuse_prefix_rendering_beyond_the_first_block() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut stream = StreamingMarkdown::new(Options::new(CountingStyles(calls.clone())));
    let source = (0..100)
        .map(|index| format!("# Heading {index}\n"))
        .collect::<String>();
    stream.append(&source);
    assert_eq!(calls.load(Ordering::Relaxed), 100);
    let before = stream.counters();

    let update = stream.append("# Tail\n");
    let work = stream.counters() - before;
    assert!(update.replay_start > source.len() - 24);
    assert!(work.processed_source_bytes < 32);
    assert_eq!(work.parsed_events, 6);
    assert_eq!(work.rendered_events, 6);
    assert_eq!(work.recomputed_blocks, 2);
    assert_eq!(calls.load(Ordering::Relaxed), 102);
    assert_eq!(work.full_recomputations, 0);
}

#[rstest]
#[case("A [link][target].", "\n\n[target]: /destination")]
#[case("A [shortcut].", "\n\n[shortcut]: /destination")]
#[case("![image][target]", "\n\n[target]: image.png")]
#[case("A note[^n].", "\n\n[^n]: definition")]
#[case("[target]: /destination\n\nA paragraph.", "\n\n[target]")]
#[case("[^n]: definition\n\nA paragraph.", "\n\nA note[^n].")]
#[case("term\n: first\n\nnext", "\n: second")]
#[case("---\nkey: value", "\n---")]
#[case("\n \n---\nkey: value", "\n---")]
#[case("earlier\n\n---\nkey: value", "\n---")]
fn global_dependencies_replay_the_full_source(#[case] first: &str, #[case] next: &str) {
    let options = Options::default();
    let mut stream = StreamingMarkdown::new(options.clone());
    let first_update = append_checked(&mut stream, first, &options, 0);
    assert_eq!(first_update.reason, ChangeReason::GlobalDependency);
    assert_eq!(first_update.stable_rows, 0);
    let before = stream.counters();
    let update = append_checked(&mut stream, next, &options, 0);
    let work = stream.counters() - before;
    assert_eq!(update.reason, ChangeReason::GlobalDependency);
    assert_eq!(update.replay_start, 0);
    assert_eq!(update.stable_rows, 0);
    assert_eq!(work.processed_source_bytes, stream.source().len() as u64);
    assert!(work.parsed_events > 0);
    assert_eq!(work.parsed_events, work.rendered_events);
    assert_eq!(work.full_recomputations, 1);
}

#[test]
fn discovering_global_dependencies_keeps_the_previous_stability_floor() {
    let options = Options::default();
    let mut stream = StreamingMarkdown::new(options.clone());
    let first = append_checked(&mut stream, "confirmed\n\nmutable", &options, 0);
    let update = append_checked(
        &mut stream,
        "\n\nA [reference][later].",
        &options,
        first.stable_rows,
    );
    assert_eq!(update.reason, ChangeReason::GlobalDependency);
    assert_eq!(update.replay_start, 0);
    assert_eq!(update.stable_rows, first.stable_rows);
    for chunk in ["\n\n[later]: /target", "\n\nanother", "\n\nparagraph"] {
        let update = append_checked(&mut stream, chunk, &options, first.stable_rows);
        assert_eq!(update.stable_rows, first.stable_rows);
        assert_eq!(update.reason, ChangeReason::GlobalDependency);
    }
}

#[test]
fn a_new_definition_after_a_checkpoint_replays_and_keeps_all_content() {
    let options = Options::default();
    let mut stream = StreamingMarkdown::new(options.clone());
    let first = append_checked(&mut stream, "first\n\nsecond\n\nthird", &options, 0);
    let before = stream.counters();
    let update = append_checked(
        &mut stream,
        "\n\n[target]: /destination\n\n[target]",
        &options,
        first.stable_rows,
    );
    let work = stream.counters() - before;
    assert_eq!(update.reason, ChangeReason::GlobalDependency);
    assert_eq!(update.replay_start, 0);
    assert_eq!(work.full_recomputations, 1);
    let suffix = &stream.source()["first\n\nsecond\n\n".len()..];
    assert_eq!(
        work.processed_source_bytes,
        (stream.source().len() + suffix.len()) as u64
    );
    assert_eq!(
        work.parsed_events,
        (Parser::new(stream.source()).count() + Parser::new(suffix).count()) as u64
    );
    assert_eq!(work.parsed_events, work.rendered_events);
    let rendered = stream.current().to_string();
    for text in ["first", "second", "third", "target", "/destination"] {
        assert!(rendered.contains(text), "{rendered}");
    }
}

#[test]
fn finish_is_a_single_full_pass_and_nonempty_append_explicitly_reopens() {
    let options = Options::default();
    for source in ["", "hello", "first\n\nsecond", "A [link][later]."] {
        let mut stream = StreamingMarkdown::new(options.clone());
        stream.append(source);
        let before = stream.counters();
        let update = stream.finish();
        let work = stream.counters() - before;
        assert_eq!(update.reason, ChangeReason::Finish);
        assert_eq!(update.first_changed_row, None);
        assert_eq!(update.replay_start, 0);
        assert_eq!(update.stable_rows, stream.current().lines.len());
        assert_eq!(work.processed_source_bytes, source.len() as u64);
        assert_eq!(work.full_recomputations, 1);
        assert_eq!(work.parsed_events, work.rendered_events);
        let finished = stream.counters();
        assert_eq!(stream.append("").reason, ChangeReason::None);
        assert_eq!(stream.finish().reason, ChangeReason::None);
        assert_eq!(stream.counters(), finished);

        let update = stream.append("\n\n[later]: /target");
        assert_eq!(update.reason, ChangeReason::Reopen);
        assert_eq!(update.replay_start, 0);
        assert_eq!(update.stable_rows, 0);
        assert_eq!((stream.counters() - finished).full_recomputations, 1);
        assert_eq!(
            stream.current(),
            &from_str_with_options(stream.source(), &options)
        );
        assert_eq!(stream.finish().reason, ChangeReason::Finish);
        let finished = stream.counters();
        stream.finish();
        assert_eq!(stream.counters(), finished);
    }
}

#[test]
fn a_reopened_plain_document_resumes_suffix_work() {
    let mut stream = StreamingMarkdown::new(Options::default());
    stream.append("first\n\nsecond");
    stream.finish();
    assert_eq!(stream.append("\n\nthird").reason, ChangeReason::Reopen);
    let before = stream.counters();
    let update = stream.append("!");
    assert_eq!(update.reason, ChangeReason::Append);
    assert_eq!(update.replay_start, "first\n\nsecond\n\n".len());
    assert_eq!((stream.counters() - before).processed_source_bytes, 6);
    assert_eq!((stream.counters() - before).full_recomputations, 0);
}

#[rstest]
#[case("1. first\n\n2", ". second")]
#[case("1) first\n\n2", ") second")]
#[case("100. first\n\n101", ". second")]
#[case("- first\n\n-", " second")]
#[case("* first\n\n*", " second")]
fn a_partial_next_item_does_not_freeze_the_preceding_list(#[case] first: &str, #[case] next: &str) {
    let options = Options::default();
    let mut stream = StreamingMarkdown::new(options.clone());
    let first = append_checked(&mut stream, first, &options, 0);
    append_checked(&mut stream, next, &options, first.stable_rows);
}

#[derive(Clone)]
struct AlternateStyles;

impl StyleSheet for AlternateStyles {
    fn heading(&self, _: u8) -> Style {
        Style::new().yellow().bold()
    }
    fn heading_marker(&self, _: u8) -> &str {
        ""
    }
    fn code_block_fence(&self) -> &str {
        ""
    }
    fn code(&self) -> Style {
        Style::new().green()
    }
    fn table_header(&self) -> Style {
        Style::new().magenta()
    }
    fn table_border(&self) -> Style {
        Style::new().blue()
    }
    fn list_marker(&self) -> Style {
        Style::new().red()
    }
}

const CORPUS: &[&str] = &[
    "plain\nsoft  \nhard\\\nnext\n\nparagraph",
    "# heading {#id}\n\nSetext\n===\n\n---\n\nlast",
    "- tight\n  - nested\n- list\n\n  loose\n\n10. ordered",
    "1. first\n\n2. second\n\n3. third\n\nlast",
    "first\n\n1) first\n\n2) second\n\n3) third\n\nlast",
    "- first\n\n- second\n\n- third\n\nlast",
    "- [ ] first\n- [x] second\n\nlast",
    "> [!NOTE]\n> quoted\n>\n> continuation\n\noutside",
    "````rust\n```\ninside\n```\n````\n\n    indented",
    "first\n\n```rust\nfn main() {}\n```\n\nlast",
    "```unknown\r\nfirst\r\n\r\nlast\r\n```\r\n\r\nAfter",
    "```rust\r\nlet first = 1;\r\n\r\nlet last = 2;",
    "<pre>\r\nfirst\r\n\r\nlast\r\n</pre>\r\n\r\nAfter",
    "| h\\|x | `a|b` |\n| :--- | ---: |\n| one | two |\nnext",
    "first\n\n| A | B |\n| - | -: |\n| 界 | e\u{301} |\n\nlast",
    "| h |\n| - |\n| a |\nnext | wider |\n\nend",
    "> | A | B |\n> | - | - |\n> | one | two |\n\noutside",
    "- | A | B |\n  | - | - |\n  | one | two |\n\noutside",
    "[inline](destination \"title\") and ![alt](image.png)",
    "[forward][ref]\n\n[ref]: /destination \"title\"",
    "note[^label]\n\n[^label]: first\n\n    second",
    "term\n: definition\n\nnext\n: another\n\nlast",
    "~~strike~~ H~2~O x^2^ $math$ $$display$$",
    "---\r\nkey: value\r\n---\r\n\r\nAfter\r\n",
    "---\n\nkey: value\n---\n\nAfter",
    "\n \n---\nkey: value\n---\n\nAfter",
    "first\n\n---\nkey: value\n---\n\nAfter",
    "first\n\n---\n\nlast",
    "first\r\rsecond\r\rthird",
    "# first\r# second\r# third",
    "👩🏽\u{200d}💻 e\u{301} 🇨🇳\n\nlast",
];

fn check_chunks<S: StyleSheet>(source: &str, options: &Options<S>, seed: u64) {
    let boundaries: Vec<_> = source
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(source.len()))
        .collect();
    let mut stream = StreamingMarkdown::new(options.clone());
    let mut cursor = 0;
    let mut state = seed;
    let mut stable_rows = 0;
    while cursor < boundaries.len() - 1 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let step = if seed == 0 { 1 } else { state as usize % 7 + 1 };
        let next = (cursor + step).min(boundaries.len() - 1);
        let update = append_checked(
            &mut stream,
            &source[boundaries[cursor]..boundaries[next]],
            options,
            stable_rows,
        );
        stable_rows = update.stable_rows;
        cursor = next;
    }
    let final_text = stream.current().clone();
    assert_eq!(stream.finish().first_changed_row, None);
    assert_eq!(stream.current(), &final_text);
    assert_eq!(
        stream.current(),
        &from_str_with_options(source, options),
        "source={source:?}"
    );
}

#[test]
fn every_utf8_prefix_and_arbitrary_chunks_match_batch() {
    for source in CORPUS {
        for seed in 0..8 {
            check_chunks(source, &Options::default(), seed);
        }
    }
}

#[test]
fn custom_styles_images_and_existing_table_width_match_every_prefix() {
    for width in [0, 9, 23, 80] {
        for fallback in [
            ImageFallback::AltText,
            ImageFallback::Url,
            ImageFallback::AltTextAndUrl,
        ] {
            let options = Options::new(AlternateStyles)
                .image_fallback(fallback)
                .table_width(width);
            for source in CORPUS {
                check_chunks(source, &options, 0);
            }
        }
    }
}

#[cfg(feature = "highlight-code")]
#[test]
fn selected_highlighting_theme_matches_every_prefix() {
    use tui_markdown::BuiltinCodeTheme;

    let options = Options::default().code_theme(BuiltinCodeTheme::SolarizedDark);
    check_chunks(
        "before\n\n```rust\nfn main() {\n    let value = \"text\";\n}\n```\n\nafter",
        &options,
        0,
    );
}

#[rstest]
#[case("\n")]
#[case("   \n\t \n")]
#[case("\r\n\t \r\n")]
fn replay_after_a_table_preserves_rows_styles_and_spacing(#[case] separator: &str) {
    let options = Options::new(AlternateStyles).table_width(18);
    let table = format!(
        "| Name | Value |\n| - | - |\n{}",
        (0..32)
            .map(|index| format!("| row {index} | wider value {index} |\n"))
            .collect::<String>()
    );
    let mut stream = StreamingMarkdown::new(options.clone());
    let first = append_checked(&mut stream, &format!("{table}{separator}tail"), &options, 0);
    let before = stream.counters();
    let update = append_checked(&mut stream, "!", &options, first.stable_rows);
    assert!(update.replay_start >= table.len(), "{update:?}");
    assert_eq!((stream.counters() - before).processed_source_bytes, 5);
    assert_eq!((stream.counters() - before).recomputed_blocks, 1);
    assert_eq!(
        update.first_changed_row,
        Some(stream.current().lines.len() - 1)
    );
    let rendered = stream.current().to_string();
    for index in 0..32 {
        assert!(rendered.contains(&format!("row {index}")), "{rendered}");
    }
    assert!(rendered.ends_with("tail!"));
    assert!(stream
        .current()
        .lines
        .iter()
        .flat_map(|line| &line.spans)
        .any(|span| span.style.fg == Some(Color::Blue)));
}
