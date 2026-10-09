//! Incremental Markdown with an owned source and cached output.

use std::mem::take;
use std::ops::Sub;

use ratatui_core::text::{Line, Text};

use crate::renderer::streaming::{render_streaming, Checkpoint, Rendered};
use crate::{DefaultStyleSheet, Options, StyleSheet};

/// The operation or dependency responsible for an update.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum ChangeReason {
    /// No work was needed.
    #[default]
    None,
    /// Source was appended from the saved checkpoint, which can still be zero.
    Append,
    /// A reference, definition list, footnote, or thematic rule required full-source processing.
    GlobalDependency,
    /// Finish performed a full-source pass.
    Finish,
    /// Nonempty input after finish reopened the document with a full-source pass.
    Reopen,
}

/// Changes to the complete output after an operation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Update {
    /// First differing output row, or `None` when all rows are unchanged.
    pub first_changed_row: Option<usize>,
    /// Initial rows that future appends will not change, until [`ChangeReason::Reopen`].
    pub stable_rows: usize,
    /// Original UTF-8 byte offset used to produce this update, or the saved offset for a no-op.
    pub replay_start: usize,
    /// Why the update was performed.
    pub reason: ChangeReason,
}

/// Cumulative work. Subtract earlier counters to inspect an operation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkCounters {
    /// Source bytes passed to the parser, including repeated work.
    pub processed_source_bytes: u64,
    /// Events obtained from the parser.
    pub parsed_events: u64,
    /// Events processed by the shared renderer.
    pub rendered_events: u64,
    /// Top-level blocks processed by the renderer.
    pub recomputed_blocks: u64,
    /// Full-source passes for global dependencies, finish, or reopen.
    ///
    /// Ordinary appends starting at zero are not included.
    pub full_recomputations: u64,
}

impl Sub for WorkCounters {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self {
            processed_source_bytes: self.processed_source_bytes - rhs.processed_source_bytes,
            parsed_events: self.parsed_events - rhs.parsed_events,
            rendered_events: self.rendered_events - rhs.rendered_events,
            recomputed_blocks: self.recomputed_blocks - rhs.recomputed_blocks,
            full_recomputations: self.full_recomputations - rhs.full_recomputations,
        }
    }
}

/// Render ordered UTF-8 fragments using the same options and semantics as batch rendering.
///
/// Completed blocks are reused; the mutable suffix is parsed and rendered again. References,
/// definition lists, footnotes, and rules that may become metadata conservatively require full-source
/// passes on subsequent appends. Long unfinished blocks can also require substantial repeated work.
///
/// ```
/// use tui_markdown::{Options, StreamingMarkdown};
///
/// let mut document = StreamingMarkdown::new(Options::default());
/// document.append("Hello **");
/// document.append("world**");
/// assert_eq!(document.current().to_string(), "Hello world");
/// document.finish();
/// ```
pub struct StreamingMarkdown<S: StyleSheet = DefaultStyleSheet> {
    options: Options<S>,
    source: String,
    current: Text<'static>,
    checkpoint: Checkpoint,
    counters: WorkCounters,
    global_dependency: bool,
    stable_rows: usize,
    finished: bool,
}

impl<S: StyleSheet> StreamingMarkdown<S> {
    /// Create an independent empty document without parsing or rendering.
    pub fn new(options: Options<S>) -> Self {
        Self {
            options,
            source: String::new(),
            current: Text::default(),
            checkpoint: Checkpoint::default(),
            counters: WorkCounters::default(),
            global_dependency: false,
            stable_rows: 0,
            finished: false,
        }
    }

    /// Append only the new fragment. Empty input does no work.
    ///
    /// Nonempty input after finish reports [`ChangeReason::Reopen`] and ends the previous
    /// stability promise.
    pub fn append(&mut self, chunk: &str) -> Update {
        if chunk.is_empty() {
            return self.unchanged();
        }
        self.source.push_str(chunk);
        let reason = if self.finished {
            self.finished = false;
            self.stable_rows = 0;
            ChangeReason::Reopen
        } else if self.global_dependency {
            ChangeReason::GlobalDependency
        } else {
            ChangeReason::Append
        };
        self.update(reason)
    }

    /// Borrow the complete cached output without parsing, rendering, or cloning.
    ///
    /// The reference borrows this document; `'static` describes the owned span contents.
    pub fn current(&self) -> &Text<'static> {
        &self.current
    }

    /// Borrow the exact source supplied so far.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Perform one fresh full-source pass and mark every output row stable.
    ///
    /// Repeated finish and empty append do no work. A later nonempty append reopens the document.
    pub fn finish(&mut self) -> Update {
        if self.finished {
            return self.unchanged();
        }
        let update = self.update(ChangeReason::Finish);
        self.finished = true;
        self.stable_rows = self.current.lines.len();
        Update {
            stable_rows: self.stable_rows,
            ..update
        }
    }

    /// Read accumulated work without exposing source content.
    pub const fn counters(&self) -> WorkCounters {
        self.counters
    }

    fn unchanged(&self) -> Update {
        Update {
            first_changed_row: None,
            stable_rows: self.stable_rows,
            replay_start: self.checkpoint.source_offset,
            reason: ChangeReason::None,
        }
    }

    fn update(&mut self, mut reason: ChangeReason) -> Update {
        let mut replay = if reason == ChangeReason::Append {
            self.checkpoint
        } else {
            Checkpoint::default()
        };
        let mut prefix = take(&mut self.current);
        let mut old_suffix = prefix.lines.split_off(replay.row);
        let mut pass = self.run_pass(replay, prefix);

        if pass.global_dependency {
            self.global_dependency = true;
            if reason == ChangeReason::Append {
                reason = ChangeReason::GlobalDependency;
            }
            if replay.source_offset != 0 {
                // Recover the unchanged prefix for comparison before retrying with document-wide
                // context. No candidate suffix is published.
                let mut old = pass.text.lines.drain(..replay.row).collect::<Vec<_>>();
                old.append(&mut old_suffix);
                old_suffix = old;
                replay = Checkpoint::default();
                pass = self.run_pass(replay, Text::default());
            }
        }
        if reason != ChangeReason::Append {
            self.counters.full_recomputations += 1;
        }

        let first_changed_row = first_changed_row(&old_suffix, &pass.text.lines[replay.row..])
            .map(|row| replay.row + row);
        self.current = pass.text;
        self.checkpoint = if self.global_dependency {
            // Keep previously promised rows stable, but do not promote any more.
            Checkpoint::default()
        } else {
            self.stable_rows = pass.checkpoint.row;
            pass.checkpoint
        };
        Update {
            first_changed_row,
            stable_rows: self.stable_rows,
            replay_start: replay.source_offset,
            reason,
        }
    }

    fn run_pass(&mut self, replay: Checkpoint, prefix: Text<'static>) -> Rendered {
        let pass = render_streaming(
            &self.source[replay.source_offset..],
            &self.options,
            replay,
            prefix,
        );
        self.counters.processed_source_bytes += pass.work.processed_source_bytes;
        self.counters.parsed_events += pass.work.parsed_events;
        self.counters.rendered_events += pass.work.rendered_events;
        self.counters.recomputed_blocks += pass.work.recomputed_blocks;
        pass
    }
}

fn first_changed_row(old: &[Line<'_>], new: &[Line<'_>]) -> Option<usize> {
    old.iter()
        .zip(new)
        .position(|(old, new)| old != new)
        .or_else(|| (old.len() != new.len()).then_some(old.len().min(new.len())))
}
