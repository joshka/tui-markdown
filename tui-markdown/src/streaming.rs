//! Render Markdown as text arrives and reuse output that has not changed.

use std::mem::{size_of, take};
use std::ops::Sub;

use ratatui_core::text::{Line, Span, Text};

use crate::layout::{wrap_text_with_checkpoint, LayoutOptions};
use crate::renderer::{render_streaming, RenderPrefix, StreamingRender};
use crate::{DefaultStyleSheet, InvalidReplacementCharacter, Options, StyleSheet, TableLimits};

/// The operation or dependency reported by a document update.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum ChangeReason {
    /// The operation did not change source, options, or output.
    #[default]
    None,
    /// Text was appended and processing started at the saved replay checkpoint.
    ///
    /// The checkpoint can be zero, so this does not always mean less than the full source was read.
    Append,
    /// Text was appended after [`StreamingMarkdown::finish`].
    ///
    /// The document accepts text again and no longer promises to keep the previous output stable.
    Reopen,
    /// The complete source was replaced.
    Replace,
    /// The document was cleared.
    Clear,
    /// A layout-only setter, such as [`StreamingMarkdown::set_width`], changed a value.
    Layout,
    /// Rendering options changed.
    Options,
    /// [`StreamingMarkdown::finish`] rendered the full source for the current source and options.
    Finish,
    /// A reference, footnote, or unresolved dependency required processing the full source.
    GlobalDependency,
}

/// Information about an update, including where a UI may need to redraw.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Update {
    /// First output row that differs, counting from zero, or `None` if output is unchanged.
    ///
    /// This refers to rows after any width wrapping, not to source lines.
    pub first_changed_row: Option<usize>,
    /// Number of initial rows that later ordinary appends will not change.
    ///
    /// Use this when writing output that cannot be revised. Replacement, clearing, options or
    /// layout changes, and [`ChangeReason::Reopen`] end this promise for the previous output.
    pub stable_rows: usize,
    /// Original UTF-8 byte offset where this update started processing source.
    pub replay_start: usize,
    /// The operation or dependency that produced this update.
    pub reason: ChangeReason,
}

/// Counts of parsing and rendering work performed by a document.
///
/// Read these with [`StreamingMarkdown::counters`] before and after an operation, then subtract
/// to measure its work. The counters contain no source text and are not reset by `clear`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkCounters {
    /// Total source bytes passed to the parser, including bytes processed more than once.
    pub processed_source_bytes: u64,
    /// Total pulldown-cmark events processed.
    pub parsed_events: u64,
    /// Total events processed by the shared Markdown renderer.
    pub rendered_events: u64,
    /// Total top-level blocks rendered, including blocks rendered more than once.
    pub recomputed_blocks: u64,
    /// Number of explicit full-source parsing and rendering passes.
    ///
    /// Ordinary append processing that happens to start at byte zero is not counted here; use
    /// [`Self::processed_source_bytes`] and event counts to measure the total work.
    pub full_recomputations: u64,
    /// Number of layout-only setting changes that rearranged or rebuilt the output.
    pub layout_reflows: u64,
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
            layout_reflows: self.layout_reflows.saturating_sub(rhs.layout_reflows),
        }
    }
}

/// Storage currently kept by a document, available through [`StreamingMarkdown::resource_usage`].
///
/// Use these values to inspect source and output buffers. They do not include every temporary
/// allocation and do not impose a total-memory limit. Longer input can require more storage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceUsage {
    /// Number of bytes in the stored UTF-8 source.
    pub source_bytes: usize,
    /// Capacity in bytes of the source buffer.
    pub source_capacity_bytes: usize,
    /// Number of text bytes in the current rendered output.
    pub current_bytes: usize,
    /// Tracked capacity in bytes of the current output, including its lines and spans.
    pub current_capacity_bytes: usize,
    /// Tracked capacity in bytes of the unwrapped output kept for later layout changes.
    pub semantic_capacity_bytes: usize,
    /// Number of saved replay checkpoints used to resume parsing.
    pub checkpoint_count: usize,
    /// Counts of tables shown as vertical lists of cells, grouped by the reason.
    pub table_fallbacks: TableFallbacks,
}

/// Counts of tables shown as vertical lists of numbered cells instead of grids.
///
/// Grids wrap their cells when possible. This presentation keeps cell content when even a grid's
/// minimum geometry cannot fit, or when a grid would exceed a cell or buffer limit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TableFallbacks {
    /// Number of tables whose minimum grid geometry did not fit the supplied body width.
    ///
    /// Minimum geometry includes borders, one-space padding, and enough width in each column
    /// for its widest complete grapheme. A wrapped grid does not count as a fallback.
    pub width: usize,
    /// Number of tables that could not use a grid within the configured cell limit.
    pub cell_limit: usize,
    /// Number of tables that could not use a grid within the configured buffer-byte limit.
    pub buffer_limit: usize,
}

/// A read-only view of rows selected by [`StreamingMarkdown::prepare_rows`].
///
/// The rows are already rendered. Reading them does not parse, render, clone, or allocate.
/// Your UI can use the same rows for display, size calculations, text selection, and mouse clicks.
/// This type supplies data; it does not perform those UI operations.
///
/// Rust prevents changes to the document while the borrowed rows are still in use.
#[derive(Clone, Copy, Debug)]
pub struct PreparedRows<'a> {
    first_row: usize,
    total_rows: usize,
    rows: &'a [Line<'static>],
}

impl<'a> PreparedRows<'a> {
    /// Returns the zero-based start row, limited to the total number of output rows.
    pub const fn first_row(&self) -> usize {
        self.first_row
    }

    /// Returns the number of rows in the complete output, not just this view.
    pub const fn total_rows(&self) -> usize {
        self.total_rows
    }

    /// Returns the selected rows without copying them.
    pub const fn rows(&self) -> &'a [Line<'static>] {
        self.rows
    }

    /// Returns whether no rows were selected.
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

/// Parse and render Markdown incrementally as text arrives.
///
/// # Using a document
///
/// - Create one object per independent document or response.
/// - Call [`Self::append`] with each new, ordered UTF-8 fragment.
/// - Read all current output with [`Self::current`], or select rows with [`Self::prepare_rows`].
/// - Call [`Self::finish`] when input ends.
///
/// The object keeps the exact source. Each update finishes before the method returns.
/// Output is styled Ratatui [`Text`], not terminal escape codes or an image; your UI draws it.
/// The caller handles network decoding, input order, and when text becomes visible.
/// A fragment may end inside Markdown syntax or between code points of a displayed character.
///
/// ```
/// use tui_markdown::{Options, StreamingMarkdown};
///
/// let mut document = StreamingMarkdown::new(Options::default().width(Some(80)));
/// document.append("Hello ");
/// let update = document.append("**world**.");
/// assert_eq!(document.current().to_string(), "Hello world.");
/// assert!(update.first_changed_row.is_some());
/// assert_eq!(document.source(), "Hello **world**.");
/// document.finish();
/// ```
///
/// # How work is reused
///
/// The object saves a replay checkpoint: the source position where parsing needs to resume.
/// Ordinary append reuses earlier output and processes the affected suffix from that position.
/// An unfinished paragraph, list, table, or code block may need to be processed again.
/// If the checkpoint is still zero, the suffix is the whole current document.
///
/// Some operations still process the full source:
///
/// - References, footnotes, and other document-wide dependencies require full-source processing.
///   Later appends keep using it until [`Self::replace`] supplies different source or
///   [`Self::clear`] removes the source.
/// - Replacing the source, replacing all options, and changing layout when tables are present
///   rebuild the output. Layout-only changes without tables rearrange cached output without parsing.
/// - [`Self::finish`] renders the full source once for the current source and options.
///
/// Every intermediate result must match fresh batch rendering with the same options.
/// Finish is not a way to hide inaccurate results while text is arriving.
///
/// # Output that cannot be revised
///
/// [`Update::stable_rows`] counts the initial rows that later ordinary appends will not change.
/// This promise ends when source is replaced or cleared, or options or layout change.
/// Nonempty append after finish reports [`ChangeReason::Reopen`] and also ends the old promise.
/// Do not treat all currently readable rows as permanently stable.
pub struct StreamingMarkdown<S: StyleSheet = DefaultStyleSheet> {
    options: Options<S>,
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
    /// Creates an empty document using the same [`Options`] as batch rendering.
    ///
    /// Default options do not wrap long lines to a width. Use [`Options::width`] to supply the
    /// available space in terminal cells. Construction does not parse or render any source.
    pub fn new(options: Options<S>) -> Self {
        Self {
            options,
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

    /// Adds a new UTF-8 fragment and updates the rendered output.
    ///
    /// Pass only the new fragment, not the accumulated source. Read the complete updated output
    /// with [`Self::current`] after this method returns. Empty input does no work.
    ///
    /// Ordinary append starts at the saved checkpoint. That position may still be zero for an
    /// unfinished construct. References, footnotes, and other dependencies can also require
    /// full-source processing on this and later appends.
    ///
    /// Nonempty input after [`Self::finish`] starts accepting text again, processes the full source,
    /// and reports [`ChangeReason::Reopen`]. Previously finished rows are no longer promised stable.
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

    /// Replaces all source text and rebuilds the output when the text differs.
    ///
    /// - Identical source does no work.
    /// - Different nonempty source is parsed and rendered in full.
    /// - Empty source removes the stored output without parsing.
    ///
    /// A change resets the replay checkpoint and document-wide dependency tracking.
    /// Do not carry the previous output's stability promise into the replacement document.
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

    /// Removes the source, output, and replay state so the object can receive a new document.
    ///
    /// An already empty document does no work. Options and accumulated work counters are kept.
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

    /// Changes the available text width without replacing your styles.
    ///
    /// - `None` disables width wrapping and table layout limits.
    /// - `Some(0)` produces no display rows.
    /// - Passing the current width does no work.
    /// - A changed width rearranges cached output without parsing when there are no tables.
    ///   With tables, the full source is parsed and rendered to rebuild their layout.
    ///
    /// The source is kept. A change makes the document unfinished again and ends the previous
    /// output's stability promise. It does not turn later ordinary appends into unconditional
    /// full-source processing.
    pub fn set_width(&mut self, width: Option<u16>) -> Update {
        self.update_layout(self.options.layout.with_width(width))
    }

    /// Changes table buffer limits without replacing your styles.
    ///
    /// Passing the current limits does no work. A change uses the same update rules as
    /// [`Self::set_width`]: without tables, it does not parse again; with tables, it rebuilds them.
    /// Limits are stored but do not affect table presentation when width is `None`.
    pub fn set_table_limits(&mut self, limits: TableLimits) -> Update {
        self.update_layout(self.options.layout.table_limits(limits))
    }

    /// Changes the character shown when one grapheme is wider than the entire available row.
    ///
    /// Passing the current value does no work. A change uses the same update rules as
    /// [`Self::set_width`], without replacing your styles. See
    /// [`Options::with_wide_grapheme_replacement`] for how the replacement is displayed.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidReplacementCharacter`] for non-printable or non-ASCII characters.
    /// Invalid input leaves source, options, output, counters, and completion unchanged.
    pub fn set_wide_grapheme_replacement(
        &mut self,
        replacement: char,
    ) -> Result<Update, InvalidReplacementCharacter> {
        let layout = self
            .options
            .layout
            .with_wide_grapheme_replacement(replacement)?;
        Ok(self.update_layout(layout))
    }

    fn update_layout(&mut self, layout: LayoutOptions) -> Update {
        if self.options.layout == layout {
            return self.unchanged();
        }
        self.options.layout = layout;
        self.bump_version();
        self.stable_rows_floor = 0;
        self.counters.layout_reflows += 1;
        if self.current_table_count == 0 {
            self.reflow_without_parse()
        } else {
            self.render_full(ChangeReason::Layout)
        }
    }

    /// Replaces all options and parses and renders the full source.
    ///
    /// The method does not compare options for equality. It always rebuilds output, even if the
    /// values would produce the same result. Use [`Self::set_width`] and the other layout-only
    /// setters to avoid replacing styles when only layout changes.
    ///
    /// This makes the document unfinished again and ends the previous output's stability promise.
    pub fn set_options(&mut self, options: Options<S>) -> Update {
        self.options = options;
        self.bump_version();
        self.stable_rows_floor = 0;
        self.render_full(ChangeReason::Options)
    }

    /// Marks input as finished and renders the full source once for the current source and options.
    ///
    /// Call this when no more text is expected, then read the result with [`Self::current`].
    /// Repeated calls with no source or options change do no work. This performs a fresh render;
    /// it does not run a separate comparison or validation test.
    ///
    /// Other operations can also process the full source; see [`StreamingMarkdown`].
    /// The object remains usable. A later nonempty append reports [`ChangeReason::Reopen`].
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

    /// Returns a read-only reference to the complete current rendered output.
    ///
    /// This includes all source supplied so far, even if Markdown syntax is unfinished.
    /// Reading does not parse, render, allocate, or clone. Your UI still draws the returned text.
    /// The reference is tied to this document; `'static` describes its owned span contents,
    /// not how long you can keep the reference.
    pub fn current(&self) -> &Text<'static> {
        &self.current
    }

    /// Returns a read-only view of a chosen range of already rendered rows.
    ///
    /// - `first_row` is zero-based and `row_count` is a count, not an end index.
    /// - Rows refer to the output after any wrapping, not to source lines.
    /// - Ranges past the end return only available rows. A start at or beyond the end returns none.
    /// - The caller chooses the range. This method does not know which rows your UI can see.
    ///
    /// Reading does not parse, render, allocate, or clone. Use the same rows to display text,
    /// calculate sizes, and handle text selection and mouse clicks. Those operations belong to
    /// the UI, not to this method.
    ///
    /// The complete output is prepared before you read it. Choosing a range does not restrict
    /// initial rendering to those rows. New input or a width change can move text to different rows.
    ///
    /// # Example
    ///
    /// ```
    /// use tui_markdown::{Options, StreamingMarkdown};
    ///
    /// let mut document = StreamingMarkdown::new(Options::default().width(Some(80)));
    /// document.append("one\n\ntwo\n\nthree");
    /// let window = document.prepare_rows(2, 3);
    /// assert_eq!(window.rows(), &document.current().lines[2..5]);
    /// assert!(document.prepare_rows(100, 10).is_empty());
    /// ```
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

    /// Returns a read-only reference to the exact source supplied so far.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns accumulated work counters without including any source text.
    pub const fn counters(&self) -> WorkCounters {
        self.counters
    }

    /// Reports the source and output storage currently kept by the document.
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
            self.options.layout,
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
            wrap_text_with_checkpoint(projection, self.options.layout, self.replay.semantic_row);
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
            reason: ChangeReason::Layout,
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
