//! Parser-derived replay boundaries for the shared event renderer.

use std::cell::Cell;
use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag};
use ratatui_core::text::{Line, Span, Text};

use super::{parser_options, TextWriter};
use crate::{Options, StyleSheet, WorkCounters};

#[derive(Clone, Copy, Default)]
pub(crate) struct Checkpoint {
    pub source_offset: usize,
    pub row: usize,
    pub needs_newline: bool,
}

pub(crate) struct Rendered {
    pub text: Text<'static>,
    pub checkpoint: Checkpoint,
    pub work: WorkCounters,
    pub global_dependency: bool,
}

struct OffsetEvents<I> {
    inner: I,
    start: usize,
}

#[derive(Clone, Copy)]
enum BlockKind {
    Table,
    List,
    Other,
}

impl<'a, I: Iterator<Item = (Event<'a>, Range<usize>)>> Iterator for OffsetEvents<I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let (event, range) = self.inner.next()?;
        self.start = range.start;
        Some(event)
    }
}

pub(crate) fn render_streaming<S: StyleSheet>(
    input: &str,
    options: &Options<S>,
    replay: Checkpoint,
    prefix: Text<'static>,
) -> Rendered {
    let unresolved = Cell::new(false);
    let parser = Parser::new_with_broken_link_callback(
        input,
        parser_options(),
        Some(|_| {
            unresolved.set(true);
            None
        }),
    );
    let mut global_dependency = parser.reference_definitions().iter().next().is_some();
    let events = OffsetEvents {
        inner: parser.into_offset_iter(),
        start: 0,
    };
    let mut writer = TextWriter::with_prefix(
        events,
        options.styles.clone(),
        options.image_fallback,
        prefix,
        replay.needs_newline,
    );
    writer.table_width = options.table_width;
    #[cfg(feature = "highlight-code")]
    let mut writer = writer.with_code_theme(options.selected_code_theme());
    let mut checkpoint = replay;
    let mut depth = 0usize;
    let mut previous_block: Option<(usize, BlockKind)> = None;
    let mut work = WorkCounters {
        processed_source_bytes: input.len() as u64,
        ..WorkCounters::default()
    };
    while let Some(event) = writer.iter.next() {
        work.parsed_events += 1;
        // At depth zero, starts and rules identify top-level parser blocks. Keep the last
        // block mutable: the parser also closes incomplete constructs at the current EOF.
        if depth == 0 && matches!(event, Event::Start(_) | Event::Rule) {
            let start = input[..writer.iter.start]
                .rfind(['\n', '\r'])
                .map_or(0, |newline| newline + 1);
            if matches!(event, Event::Rule) {
                global_dependency = true;
            }
            // Keep the first block's replay context, including leading whitespace. A following
            // unfinished paragraph can still become a table row or another ordered list item.
            let can_advance = previous_block.is_some_and(|(offset, kind)| match kind {
                BlockKind::Table => has_blank_line(&input[offset..start]),
                BlockKind::List => !matches!(event, Event::Start(Tag::Paragraph)),
                BlockKind::Other => true,
            });
            if can_advance {
                checkpoint = Checkpoint {
                    source_offset: replay.source_offset + start,
                    row: writer.text.lines.len(),
                    needs_newline: writer.needs_newline,
                };
            }
            let kind = match &event {
                Event::Start(Tag::Table(_)) => BlockKind::Table,
                Event::Start(Tag::List(_)) => BlockKind::List,
                _ => BlockKind::Other,
            };
            previous_block = Some((start, kind));
            work.recomputed_blocks += 1;
        }
        match &event {
            Event::Start(tag) => {
                depth += 1;
                global_dependency |=
                    matches!(tag, Tag::FootnoteDefinition(_) | Tag::DefinitionList);
            }
            Event::End(_) => depth -= 1,
            Event::FootnoteReference(_) => global_dependency = true,
            // Unresolved footnotes remain text and do not invoke the broken-link callback.
            Event::Text(text)
                if text.contains("[^") || input[writer.iter.start..].starts_with("[^") =>
            {
                global_dependency = true;
            }
            _ => {}
        }
        writer.handle_event(event);
        work.rendered_events += 1;
    }
    Rendered {
        text: own_text(writer.text),
        checkpoint,
        work,
        global_dependency: global_dependency || unresolved.get(),
    }
}

fn has_blank_line(source: &str) -> bool {
    source.split_inclusive('\n').any(|line| {
        line.strip_suffix('\n').is_some_and(|line| {
            line.trim_end_matches('\r')
                .bytes()
                .all(|byte| matches!(byte, b' ' | b'\t'))
        })
    })
}

fn own_text(text: Text<'_>) -> Text<'static> {
    Text {
        style: text.style,
        alignment: text.alignment,
        lines: text
            .lines
            .into_iter()
            .map(|line| Line {
                style: line.style,
                alignment: line.alignment,
                spans: line
                    .spans
                    .into_iter()
                    .map(|span| Span::styled(span.content.into_owned(), span.style))
                    .collect(),
            })
            .collect(),
    }
}
