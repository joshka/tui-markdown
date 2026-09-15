//! Stateful incremental Markdown rendering.

use std::mem::{size_of, take};
use std::ops::Sub;

use ratatui_core::text::{Line, Span, Text};

use crate::layout::wrap_text_with_checkpoint;
use crate::renderer::{render_streaming, RenderPrefix, StreamingRender};
use crate::{DefaultStyleSheet, Options, RenderContext, StyleSheet};

/// Why a streaming snapshot changed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum ChangeReason {
    /// The operation did not change source, options, context, or output.
    #[default]
    None,
    /// Ordered source was appended using suffix replay.
    Append,
    /// Source was appended after completion, explicitly reopening a new mutable lineage.
    Reopen,
    /// The complete source was replaced.
    Replace,
    /// The document was cleared.
    Clear,
    /// Rendering context changed.
    Context,
    /// Rendering options changed.
    Options,
    /// A fresh canonical completion pass was performed.
    Finish,
    /// A reference, footnote, or unresolved dependency required document-wide recomputation.
    GlobalDependency,
}

/// Metadata returned by every mutation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Update {
    /// Earliest display row that differs, or `None` when display output is unchanged.
    pub first_changed_row: Option<usize>,
    /// Display rows safe for irreversible output in the current append lineage.
    pub stable_rows: usize,
    /// Original UTF-8 byte offset where this operation began replaying source.
    pub replay_start: usize,
    /// The operation or invalidation that produced this update.
    pub reason: ChangeReason,
}

/// Cumulative, source-free work diagnostics for one document.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkCounters {
    /// Source bytes submitted to parser passes, including explicit full recomputations.
    pub processed_source_bytes: u64,
    /// Pulldown-cmark events consumed.
    pub parsed_events: u64,
    /// Events submitted to the canonical renderer.
    pub rendered_events: u64,
    /// Top-level blocks recomputed.
    pub recomputed_blocks: u64,
    /// Fresh whole-document parser and renderer passes.
    pub full_recomputations: u64,
    /// Display reflows caused by context changes.
    pub context_reflows: u64,
}

impl Sub for WorkCounters {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            processed_source_bytes: self
                .processed_source_bytes
                .saturating_sub(rhs.processed_source_bytes),
            parsed_events: self.parsed_events.saturating_sub(rhs.parsed_events),
            rendered_events: self.rendered_events.saturating_sub(rhs.rendered_events),
            recomputed_blocks: self.recomputed_blocks.saturating_sub(rhs.recomputed_blocks),
            full_recomputations: self
                .full_recomputations
                .saturating_sub(rhs.full_recomputations),
            context_reflows: self.context_reflows.saturating_sub(rhs.context_reflows),
        }
    }
}

/// Current authoritative and derived allocation accounting.
///
/// These values describe retained state; they are not a total-memory cap. The authoritative source
/// and returned snapshot necessarily scale with input.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceUsage {
    /// Bytes of authoritative UTF-8 source.
    pub source_bytes: usize,
    /// Allocated capacity of authoritative UTF-8 source.
    pub source_capacity_bytes: usize,
    /// Bytes of visible span content.
    pub current_bytes: usize,
    /// Accounted allocation capacity of the current display snapshot.
    pub current_capacity_bytes: usize,
    /// Accounted allocation capacity of the retained unwrapped renderer snapshot.
    pub semantic_capacity_bytes: usize,
    /// Fixed replay checkpoints currently retained.
    pub checkpoint_count: usize,
    /// Stacked-table fallbacks retained in the current snapshot.
    pub table_fallbacks: TableFallbacks,
}

/// Reasons grid tables use the content-preserving stacked presentation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TableFallbacks {
    /// Tables whose grid geometry exceeded the supplied body width.
    pub width: usize,
    /// Tables whose grid buffering reached the configured cell limit.
    pub cell_limit: usize,
    /// Tables whose grid buffering reached the configured allocation-byte limit.
    pub buffer_limit: usize,
}

/// A borrowed window into rows already prepared for the current display snapshot.
///
/// This view performs no parsing, rendering, cloning, or allocation. Mutation of its originating
/// [`StreamingMarkdown`] is prevented while the view is borrowed, so geometry, drawing, selection,
/// and hit testing can share the same snapshot.
#[derive(Clone, Copy, Debug)]
pub struct PreparedRows<'a> {
    first_row: usize,
    total_rows: usize,
    rows: &'a [Line<'static>],
}

impl<'a> PreparedRows<'a> {
    /// Returns the clamped display-row offset of this window.
    pub const fn first_row(&self) -> usize {
        self.first_row
    }

    /// Returns the complete snapshot's display-row count.
    pub const fn total_rows(&self) -> usize {
        self.total_rows
    }

    /// Borrows the prepared rows in this window.
    pub const fn rows(&self) -> &'a [Line<'static>] {
        self.rows
    }

    /// Returns whether this window contains no rows.
    pub const fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ReplayCheckpoint {
    source_offset: usize,
    semantic_row: usize,
    display_row: usize,
    needs_newline: bool,
    table_fallbacks: [usize; 3],
    table_count: usize,
}

/// A consumer-owned streaming Markdown document.
pub struct StreamingMarkdown<S: StyleSheet = DefaultStyleSheet> {
    options: Options<S>,
    context: RenderContext,
    source: String,
    semantic: Text<'static>,
    current: Text<'static>,
    current_table_fallbacks: [usize; 3],
    current_table_count: usize,
    replay: ReplayCheckpoint,
    counters: WorkCounters,
    global_dependency: bool,
    stable_rows_floor: usize,
    version: u64,
    finished_version: Option<u64>,
}

impl<S: StyleSheet> StreamingMarkdown<S> {
    /// Creates an empty document with the supplied rendering options and body context.
    pub fn new(options: Options<S>, context: RenderContext) -> Self {
        Self {
            options,
            context,
            source: String::new(),
            semantic: Text::default(),
            current: Text::default(),
            current_table_fallbacks: [0; 3],
            current_table_count: 0,
            replay: ReplayCheckpoint::default(),
            counters: WorkCounters::default(),
            global_dependency: false,
            stable_rows_floor: 0,
            version: 0,
            finished_version: None,
        }
    }

    /// Appends ordered UTF-8 source and incrementally replaces the affected display suffix.
    pub fn append(&mut self, chunk: &str) -> Update {
        if chunk.is_empty() {
            return self.unchanged();
        }
        let had_source = !self.source.is_empty();
        let reopening = self.finished_version == Some(self.version);
        self.source.push_str(chunk);
        self.bump_version();
        if reopening {
            self.replay = ReplayCheckpoint::default();
            self.stable_rows_floor = 0;
            return self.render_full(ChangeReason::Reopen);
        }
        let reason = if self.global_dependency {
            self.counters.full_recomputations += 1;
            ChangeReason::GlobalDependency
        } else {
            ChangeReason::Append
        };
        self.render_from_replay(reason, had_source)
    }

    /// Replaces the complete source and discards incompatible incremental state.
    pub fn replace(&mut self, source: &str) -> Update {
        if self.source == source {
            return self.unchanged();
        }
        let display_changed = !self.current.lines.is_empty();
        self.source.clear();
        self.source.push_str(source);
        self.reset_projection();
        self.bump_version();
        if source.is_empty() {
            return Update {
                first_changed_row: display_changed.then_some(0),
                stable_rows: 0,
                replay_start: 0,
                reason: ChangeReason::Replace,
            };
        }
        self.render_full(ChangeReason::Replace)
    }

    /// Clears source and all document-specific parser and renderer state.
    pub fn clear(&mut self) -> Update {
        if self.source.is_empty() && self.current.lines.is_empty() {
            return self.unchanged();
        }
        let changed = !self.current.lines.is_empty();
        self.source.clear();
        self.reset_projection();
        self.bump_version();
        Update {
            first_changed_row: changed.then_some(0),
            stable_rows: 0,
            replay_start: 0,
            reason: ChangeReason::Clear,
        }
    }

    /// Updates terminal body width and presentation limits.
    pub fn set_context(&mut self, context: RenderContext) -> Update {
        if self.context == context {
            return self.unchanged();
        }
        self.context = context;
        self.bump_version();
        self.stable_rows_floor = 0;
        self.counters.context_reflows += 1;
        if self.current_table_count == 0 {
            self.reflow_without_parse()
        } else {
            self.render_full(ChangeReason::Context)
        }
    }

    /// Replaces styling and rendering options and recomputes the current document.
    pub fn set_options(&mut self, options: Options<S>) -> Update {
        self.options = options;
        self.bump_version();
        self.stable_rows_floor = 0;
        self.render_full(ChangeReason::Options)
    }

    /// Performs one fresh canonical full-input pass for the current source version.
    pub fn finish(&mut self) -> Update {
        if self.finished_version == Some(self.version) {
            return self.unchanged();
        }
        let update = self.render_full(ChangeReason::Finish);
        self.finished_version = Some(self.version);
        Update {
            stable_rows: self.current.lines.len(),
            ..update
        }
    }

    /// Borrows the complete current owned display snapshot.
    pub fn current(&self) -> &Text<'static> {
        &self.current
    }

    /// Borrows a clamped range of already prepared display rows.
    ///
    /// Repeated calls reuse the current snapshot's backing storage. This lets retained UIs
    /// materialize only their viewport and overscan while [`Self::current`] continues to provide
    /// the complete canonical snapshot.
    #[must_use]
    pub fn prepare_rows(&self, first_row: usize, row_count: usize) -> PreparedRows<'_> {
        let total_rows = self.current.lines.len();
        let first_row = first_row.min(total_rows);
        let end_row = first_row.saturating_add(row_count).min(total_rows);
        PreparedRows {
            first_row,
            total_rows,
            rows: &self.current.lines[first_row..end_row],
        }
    }

    /// Borrows the exact authoritative UTF-8 source.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns cumulative non-sensitive work counters.
    pub const fn counters(&self) -> WorkCounters {
        self.counters
    }

    /// Returns retained source and derived snapshot allocation accounting.
    pub fn resource_usage(&self) -> ResourceUsage {
        ResourceUsage {
            source_bytes: self.source.len(),
            source_capacity_bytes: self.source.capacity(),
            current_bytes: text_content_bytes(&self.current),
            current_capacity_bytes: text_capacity_bytes(&self.current),
            semantic_capacity_bytes: text_capacity_bytes(&self.semantic),
            checkpoint_count: usize::from(!self.source.is_empty() && !self.global_dependency),
            table_fallbacks: TableFallbacks {
                width: self.current_table_fallbacks[0],
                cell_limit: self.current_table_fallbacks[1],
                buffer_limit: self.current_table_fallbacks[2],
            },
        }
    }

    fn bump_version(&mut self) {
        self.version = self.version.wrapping_add(1);
        self.finished_version = None;
    }

    fn unchanged(&self) -> Update {
        Update {
            first_changed_row: None,
            stable_rows: if self.finished_version == Some(self.version) {
                self.current.lines.len()
            } else if self.global_dependency {
                self.stable_rows_floor
            } else {
                self.replay.display_row
            },
            replay_start: self.replay.source_offset,
            reason: ChangeReason::None,
        }
    }

    fn reset_projection(&mut self) {
        self.semantic = Text::default();
        self.current = Text::default();
        self.current_table_fallbacks = [0; 3];
        self.current_table_count = 0;
        self.replay = ReplayCheckpoint::default();
        self.global_dependency = false;
        self.stable_rows_floor = 0;
        self.finished_version = None;
    }

    fn render_full(&mut self, reason: ChangeReason) -> Update {
        self.counters.full_recomputations += 1;
        self.replay = ReplayCheckpoint::default();
        self.render_from_replay(reason, false)
    }

    fn render_from_replay(
        &mut self,
        mut reason: ChangeReason,
        count_discovered_global_full: bool,
    ) -> Update {
        let mut used_replay = if self.global_dependency {
            ReplayCheckpoint::default()
        } else {
            self.replay
        };
        let mut old_current = take(&mut self.current);
        let mut current_prefix = Text {
            alignment: old_current.alignment,
            style: old_current.style,
            lines: old_current
                .lines
                .drain(..used_replay.display_row.min(old_current.lines.len()))
                .collect(),
        };
        let mut old_suffix = old_current.lines;
        let mut semantic_prefix = take(&mut self.semantic);
        semantic_prefix
            .lines
            .truncate(used_replay.semantic_row.min(semantic_prefix.lines.len()));

        let mut pass = self.run_pass(used_replay, semantic_prefix);
        if pass.has_global_dependency && !self.global_dependency && used_replay.source_offset != 0 {
            reason = ChangeReason::GlobalDependency;
            self.stable_rows_floor = self.stable_rows_floor.max(used_replay.display_row);
            self.global_dependency = true;
            used_replay = ReplayCheckpoint::default();
            current_prefix.lines.append(&mut old_suffix);
            old_suffix = take(&mut current_prefix.lines);
            pass = self.run_pass(used_replay, Text::default());
            self.counters.full_recomputations += 1;
        } else if pass.has_global_dependency {
            if !self.global_dependency {
                if count_discovered_global_full && used_replay.source_offset == 0 {
                    self.counters.full_recomputations += 1;
                }
                self.stable_rows_floor = self.stable_rows_floor.max(used_replay.display_row);
            }
            self.global_dependency = true;
            if reason == ChangeReason::Append {
                reason = ChangeReason::GlobalDependency;
            }
        }

        let semantic_start = used_replay.semantic_row.min(pass.text.lines.len());
        let checkpoint_semantic_row = pass
            .checkpoint
            .as_ref()
            .map_or(semantic_start, |checkpoint| checkpoint.semantic_row);
        let suffix = Text {
            alignment: pass.text.alignment,
            style: pass.text.style,
            lines: pass.text.lines[semantic_start..].to_vec(),
        };
        let (mut display_suffix, relative_display_checkpoint) = wrap_text_with_checkpoint(
            suffix,
            self.context,
            checkpoint_semantic_row.saturating_sub(semantic_start),
        );
        let prefix_rows = current_prefix.lines.len();
        let first_changed_row = first_changed_row(prefix_rows, &old_suffix, &display_suffix.lines);
        current_prefix.lines.append(&mut display_suffix.lines);
        self.current = current_prefix;
        self.semantic = pass.text;
        self.current_table_fallbacks = pass.table_fallbacks;
        self.current_table_count = pass.table_count;

        self.replay = if self.global_dependency {
            ReplayCheckpoint::default()
        } else if let Some(checkpoint) = pass.checkpoint {
            ReplayCheckpoint {
                source_offset: checkpoint.source_offset,
                semantic_row: checkpoint.semantic_row,
                display_row: prefix_rows + relative_display_checkpoint,
                needs_newline: checkpoint.needs_newline,
                table_fallbacks: checkpoint.table_fallbacks,
                table_count: checkpoint.table_count,
            }
        } else {
            used_replay
        };

        Update {
            first_changed_row,
            stable_rows: if self.global_dependency {
                self.stable_rows_floor
            } else {
                self.replay.display_row
            },
            replay_start: used_replay.source_offset,
            reason,
        }
    }

    fn run_pass(&mut self, replay: ReplayCheckpoint, prefix: Text<'static>) -> StreamingRender {
        let input = &self.source[replay.source_offset..];
        let pass = render_streaming(
            input,
            &self.options,
            self.context,
            replay.source_offset,
            RenderPrefix {
                text: prefix,
                needs_newline: replay.needs_newline,
                table_fallbacks: replay.table_fallbacks,
                table_count: replay.table_count,
            },
        );
        self.counters.processed_source_bytes += input.len() as u64;
        self.counters.parsed_events += pass.events;
        self.counters.rendered_events += pass.events;
        self.counters.recomputed_blocks += pass.blocks;
        pass
    }

    fn reflow_without_parse(&mut self) -> Update {
        let old = take(&mut self.current);
        let projection = Text {
            alignment: self.semantic.alignment,
            style: self.semantic.style,
            lines: self.semantic.lines.clone(),
        };
        let (current, display_checkpoint) =
            wrap_text_with_checkpoint(projection, self.context, self.replay.semantic_row);
        let first_changed_row = first_changed_row(0, &old.lines, &current.lines);
        self.current = current;
        self.replay.display_row = display_checkpoint;
        Update {
            first_changed_row,
            stable_rows: if self.global_dependency {
                self.stable_rows_floor
            } else {
                self.replay.display_row
            },
            replay_start: self.replay.source_offset,
            reason: ChangeReason::Context,
        }
    }
}

fn first_changed_row(prefix_rows: usize, old: &[Line<'_>], new: &[Line<'_>]) -> Option<usize> {
    let shared = old.len().min(new.len());
    for row in 0..shared {
        if old[row] != new[row] {
            return Some(prefix_rows + row);
        }
    }
    (old.len() != new.len()).then_some(prefix_rows + shared)
}

fn text_content_bytes(text: &Text<'_>) -> usize {
    text.lines
        .iter()
        .flat_map(|line| &line.spans)
        .map(|span| span.content.len())
        .sum()
}

fn text_capacity_bytes(text: &Text<'_>) -> usize {
    text.lines.capacity() * size_of::<Line<'static>>()
        + text
            .lines
            .iter()
            .map(|line| {
                line.spans.capacity() * size_of::<Span<'static>>()
                    + line
                        .spans
                        .iter()
                        .map(|span| match &span.content {
                            std::borrow::Cow::Borrowed(_) => 0,
                            std::borrow::Cow::Owned(content) => content.capacity(),
                        })
                        .sum::<usize>()
            })
            .sum::<usize>()
}
