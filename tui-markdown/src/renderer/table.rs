//! Table rendering support for tui-markdown.
//!
//! A table must be buffered before rendering because every cell can increase its column's terminal
//! display width. [`TableBuilder`] collects the header and body rows, then renders their content,
//! alignment, padding, and Unicode box-drawing borders once pulldown-cmark closes the table.
//!
//! The central renderer dispatches events and owns shared inline state. This module owns the table
//! event handlers, buffered table state, list-aware output placement, and final table layout.

use std::borrow::Cow;
use std::mem::{size_of, take};

use pulldown_cmark::Alignment;
use ratatui_core::style::Style;
use ratatui_core::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::TextWriter;
use crate::layout::LayoutOptions;
use crate::{StyleSheet, TableLimits};

const HORIZONTAL_BORDER: char = '─';
const VERTICAL_BORDER: &str = "│";
const TOP_BORDER: BorderGlyphs = BorderGlyphs::new('┌', '┬', '┐');
const HEADER_SEPARATOR: BorderGlyphs = BorderGlyphs::new('├', '┼', '┤');
const BOTTOM_BORDER: BorderGlyphs = BorderGlyphs::new('└', '┴', '┘');

impl<'a, 'theme, I, S> TextWriter<'a, 'theme, I, S>
where
    I: Iterator<Item = pulldown_cmark::Event<'a>>,
    S: StyleSheet,
{
    pub fn start_table(&mut self, alignments: Vec<Alignment>) {
        self.table_count += 1;
        if self.needs_newline {
            self.push_line(Line::default());
        }
        let mut builder = TableBuilder::new(alignments);
        if let Some(body_width) = self.context.width() {
            let prefixes = self.line_prefixes.iter().map(Span::width).sum::<usize>()
                + usize::from(!self.line_prefixes.is_empty());
            let indentation = self
                .list_items
                .last()
                .map_or(0, |item| item.continuation_width);
            let width = usize::from(body_width).saturating_sub(prefixes + indentation);
            builder.configure(self.context, width, &self.styles);
        }
        self.table_builder = Some(builder);
        self.needs_newline = false;
    }

    pub fn end_table_header(&mut self) {
        if let Some(builder) = &mut self.table_builder {
            builder.finish_header();
        }
    }

    pub fn end_table_row(&mut self) {
        if let Some(builder) = &mut self.table_builder {
            builder.finish_row();
        }
    }

    pub fn start_table_cell(&mut self) {
        if let Some(builder) = &mut self.table_builder {
            builder.start_cell();
        }
    }

    pub fn end_table_cell(&mut self) {
        if let Some(builder) = &mut self.table_builder {
            builder.finish_cell();
        }
    }

    pub fn end_table(&mut self) {
        if let Some(builder) = self.table_builder.take() {
            let (lines, fallback) = builder.render_with_fallback(&self.styles);
            if let Some(fallback) = fallback {
                self.table_fallbacks[fallback as usize] += 1;
            }
            self.push_table_lines(lines);
            self.needs_newline = true;
        }
    }

    /// Adds a buffered table to the output while preserving an active list item's layout.
    ///
    /// A table that is the first content in an item starts on the marker line. Its remaining lines
    /// are indented by the marker's display width. A later table cannot reuse the marker line, but
    /// all of its lines still need the continuation indentation.
    ///
    /// Table rendering currently puts styles on individual spans and leaves the line style and
    /// alignment at their defaults. This makes it safe to move the first rendered line's spans
    /// onto the existing marker line.
    fn push_table_lines(&mut self, lines: Vec<Line<'a>>) {
        let Some(list_item) = self.list_items.last().copied() else {
            for line in lines {
                self.push_line(line);
            }
            return;
        };

        let mut lines = lines.into_iter();
        // The line position alone is insufficient: inline item content may already have appended
        // spans to the marker line before the table was buffered.
        let marker_line_is_last = self.text.lines.len() == list_item.marker_line + 1;
        let marker_has_no_content =
            self.text.lines[list_item.marker_line].spans.len() == list_item.marker_span_count;
        let table_starts_on_marker = marker_line_is_last && marker_has_no_content;
        if table_starts_on_marker {
            if let Some(first_line) = lines.next() {
                self.text.lines[list_item.marker_line]
                    .spans
                    .extend(first_line.spans);
            }
        }

        let continuation = " ".repeat(list_item.continuation_width);
        for mut line in lines {
            line.spans.insert(0, Span::raw(continuation.clone()));
            self.push_line(line);
        }
    }
}

/// Accumulates a complete table before calculating its column widths and rendering it.
///
/// The parent renderer starts and finishes each cell as pulldown-cmark emits table events. It then
/// finishes the header or body row and calls [`Self::render`] after the table closes.
pub struct TableBuilder<'a> {
    alignments: Vec<Alignment>,
    header: TableHeader<'a>,
    rows: Vec<TableRow<'a>>,
    current_row: TableRow<'a>,
    current_cell: TableCell<'a>,
    layout: Option<TableLayout>,
    stacked_lines: Option<Vec<Line<'a>>>,
    in_header: bool,
    row_index: usize,
    column_index: usize,
    cell_open: bool,
    fallback: Option<TableFallback>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub(super) enum TableFallback {
    Width = 0,
    CellLimit = 1,
    BufferLimit = 2,
}

struct TableLayout {
    width: usize,
    limits: TableLimits,
    buffer_bytes: usize,
    cell_count: usize,
    header_style: Style,
    cell_style: Style,
}

impl TableLayout {
    fn charge(&mut self, bytes: usize) -> bool {
        if bytes
            > self
                .limits
                .max_buffer_bytes
                .saturating_sub(self.buffer_bytes)
        {
            return false;
        }
        self.buffer_bytes += bytes;
        true
    }

    fn reserve<T>(&mut self, values: &mut Vec<T>) -> bool {
        if values.len() < values.capacity() {
            return true;
        }
        let available = self
            .limits
            .max_buffer_bytes
            .saturating_sub(self.buffer_bytes)
            / size_of::<T>();
        if available == 0 {
            return false;
        }
        let additional = values.capacity().max(4).min(available);
        if !self.charge(additional * size_of::<T>()) {
            return false;
        }
        values.reserve_exact(additional);
        true
    }
}

impl<'a> TableBuilder<'a> {
    pub fn new(alignments: Vec<Alignment>) -> Self {
        Self {
            alignments,
            header: TableHeader::default(),
            rows: Vec::new(),
            current_row: TableRow::default(),
            current_cell: TableCell::default(),
            layout: None,
            stacked_lines: None,
            in_header: true,
            row_index: 0,
            column_index: 0,
            cell_open: false,
            fallback: None,
        }
    }

    fn configure<S: StyleSheet>(&mut self, context: LayoutOptions, width: usize, styles: &S) {
        let mut layout = TableLayout {
            width,
            limits: context.limits(),
            buffer_bytes: 0,
            cell_count: 0,
            header_style: styles.table_header(),
            cell_style: styles.table_cell(),
        };
        let alignment_fits = layout.charge(self.alignments.capacity() * size_of::<Alignment>());
        let minimum_grid_fits = self.alignments.len().saturating_mul(4).saturating_add(1) <= width;
        let has_cell_capacity = layout.limits.max_cells > 0;
        self.layout = Some(layout);
        if !alignment_fits {
            self.start_stacked(TableFallback::BufferLimit);
        } else if !has_cell_capacity {
            self.start_stacked(TableFallback::CellLimit);
        } else if !minimum_grid_fits {
            self.start_stacked(TableFallback::Width);
        }
    }

    pub fn start_cell(&mut self) {
        self.cell_open = true;
        self.current_cell = TableCell::default();
        if self.stacked_lines.is_some() {
            self.start_stacked_cell();
        } else if let Some(layout) = &mut self.layout {
            if layout.cell_count >= layout.limits.max_cells {
                self.start_stacked(TableFallback::CellLimit);
            } else {
                layout.cell_count += 1;
            }
        }
    }

    pub fn push_span(&mut self, span: Span<'a>) {
        if self.stacked_lines.is_none() {
            if let Some(layout) = &mut self.layout {
                let bytes = match &span.content {
                    Cow::Owned(content) => content.capacity(),
                    Cow::Borrowed(_) => 0,
                };
                if !layout.charge(bytes) || !layout.reserve(&mut self.current_cell.spans) {
                    self.start_stacked(TableFallback::BufferLimit);
                }
            }
        }
        if let Some(lines) = &mut self.stacked_lines {
            let layout = self.layout.as_ref().expect("stacked tables have a layout");
            let style = if self.in_header {
                layout.header_style
            } else {
                layout.cell_style
            };
            lines
                .last_mut()
                .expect("table cell is open")
                .push_span(span.patch_style(style));
        } else {
            self.current_cell.push(span);
        }
    }

    pub fn finish_cell(&mut self) {
        if self.stacked_lines.is_none() {
            if let Some(layout) = &mut self.layout {
                if !layout.reserve(&mut self.current_row.cells) {
                    self.start_stacked(TableFallback::BufferLimit);
                }
            }
        }
        if self.stacked_lines.is_none() {
            self.current_row.cells.push(take(&mut self.current_cell));
        }
        self.column_index += 1;
        self.cell_open = false;
    }

    pub fn finish_header(&mut self) {
        self.finish_logical_row();
        self.header.cells = std::mem::take(&mut self.current_row.cells);
        self.in_header = false;
        self.column_index = 0;
    }

    pub fn finish_row(&mut self) {
        self.finish_logical_row();
        if self.stacked_lines.is_none() {
            if let Some(layout) = &mut self.layout {
                if !layout.reserve(&mut self.rows) {
                    self.start_stacked(TableFallback::BufferLimit);
                }
            }
        }
        if self.stacked_lines.is_none() {
            self.rows.push(take(&mut self.current_row));
        }
        self.row_index += 1;
        self.column_index = 0;
    }

    fn finish_logical_row(&mut self) {
        let missing = self.alignments.len().saturating_sub(self.column_index);
        if missing == 0 {
            return;
        }

        if self.stacked_lines.is_some() {
            for _ in 0..missing {
                self.start_stacked_cell();
                self.column_index += 1;
            }
            return;
        }

        let exceeds_cell_limit = self.layout.as_ref().is_some_and(|layout| {
            missing > layout.limits.max_cells.saturating_sub(layout.cell_count)
        });
        if exceeds_cell_limit {
            self.current_row
                .cells
                .resize_with(self.alignments.len(), TableCell::default);
            self.column_index += missing;
            self.start_stacked(TableFallback::CellLimit);
            return;
        }

        for remaining in (0..missing).rev() {
            let reserved = self
                .layout
                .as_mut()
                .is_none_or(|layout| layout.reserve(&mut self.current_row.cells));
            if !reserved {
                self.current_row.cells.resize_with(
                    self.current_row.cells.len() + remaining + 1,
                    TableCell::default,
                );
                self.column_index += remaining + 1;
                self.start_stacked(TableFallback::BufferLimit);
                return;
            }
            self.current_row.cells.push(TableCell::default());
            if let Some(layout) = &mut self.layout {
                layout.cell_count += 1;
            }
            self.column_index += 1;
        }
    }

    #[cfg(test)]
    pub fn render<S: StyleSheet>(self, styles: &S) -> Vec<Line<'a>> {
        self.render_with_fallback(styles).0
    }

    pub(super) fn render_with_fallback<S: StyleSheet>(
        mut self,
        styles: &S,
    ) -> (Vec<Line<'a>>, Option<TableFallback>) {
        if let Some(lines) = self.stacked_lines.take() {
            return (lines, self.fallback);
        }
        let column_count = self.column_count();
        if column_count == 0 {
            return (Vec::new(), self.fallback);
        }

        let measurement_scratch_bytes = self.measurement_scratch_bytes();
        let scratch_fits = self.layout.as_ref().is_none_or(|layout| {
            measurement_scratch_bytes
                <= layout
                    .limits
                    .max_buffer_bytes
                    .saturating_sub(layout.buffer_bytes)
        });
        if !scratch_fits {
            self.start_stacked(TableFallback::BufferLimit);
            return (
                self.stacked_lines.take().expect("stacked table output"),
                self.fallback,
            );
        }
        if let Some(layout) = &mut self.layout {
            let charged = layout.charge(measurement_scratch_bytes);
            debug_assert!(charged, "measurement scratch was preflighted");
        }
        let mut measurement_scratch = String::with_capacity(measurement_scratch_bytes);
        let column_widths = self.column_widths(column_count, &mut measurement_scratch);
        if self.layout.as_ref().is_some_and(|layout| {
            column_widths
                .iter()
                .sum::<usize>()
                .saturating_add(column_count.saturating_mul(3))
                .saturating_add(1)
                > layout.width
        }) {
            self.start_stacked(TableFallback::Width);
            return (
                self.stacked_lines.take().expect("stacked table output"),
                self.fallback,
            );
        }
        let border_style = styles.table_border();

        let top_border = TOP_BORDER.render(&column_widths, border_style);
        let header = self.header.render(
            &column_widths,
            &self.alignments,
            styles,
            &mut measurement_scratch,
        );
        let header_separator = HEADER_SEPARATOR.render(&column_widths, border_style);
        let bottom_border = BOTTOM_BORDER.render(&column_widths, border_style);

        let mut lines = vec![top_border, header, header_separator];
        for row in &self.rows {
            lines.push(row.render(
                &column_widths,
                &self.alignments,
                styles,
                &mut measurement_scratch,
            ));
        }
        lines.push(bottom_border);
        (lines, self.fallback)
    }

    fn start_stacked_cell(&mut self) {
        let lines = self.stacked_lines.as_mut().expect("stacked table output");
        let layout = self.layout.as_ref().expect("stacked tables have a layout");
        if self.column_index == 0 {
            let row = if self.in_header {
                0
            } else {
                self.row_index + 1
            };
            push_stacked_label(lines, row);
        }
        let style = if self.in_header {
            layout.header_style
        } else {
            layout.cell_style
        };
        lines.push(Line::from(Span::styled(
            format!("[{}] ", self.column_index + 1),
            style,
        )));
    }

    fn start_stacked(&mut self, fallback: TableFallback) {
        self.fallback.get_or_insert(fallback);
        let layout = self.layout.as_mut().expect("stacked tables have a layout");
        let mut lines = Vec::new();
        if !self.in_header {
            push_stacked_row(
                &mut lines,
                0,
                take(&mut self.header.cells),
                layout.header_style,
            );
            for (index, row) in take(&mut self.rows).into_iter().enumerate() {
                push_stacked_row(&mut lines, index + 1, row.cells, layout.cell_style);
            }
        }
        if !self.current_row.cells.is_empty() || self.cell_open {
            let index = if self.in_header {
                0
            } else {
                self.row_index + 1
            };
            let style = if self.in_header {
                layout.header_style
            } else {
                layout.cell_style
            };
            let mut cells = take(&mut self.current_row.cells);
            if self.cell_open {
                cells.push(take(&mut self.current_cell));
            }
            push_stacked_row(&mut lines, index, cells, style);
        }
        self.alignments = Vec::new();
        layout.buffer_bytes = 0;
        self.stacked_lines = Some(lines);
    }

    fn column_count(&self) -> usize {
        self.alignments.len().max(self.header.cells.len()).max(
            self.rows
                .iter()
                .map(|row| row.cells.len())
                .max()
                .unwrap_or(0),
        )
    }

    fn measurement_scratch_bytes(&self) -> usize {
        self.header
            .cells
            .iter()
            .chain(self.rows.iter().flat_map(|row| &row.cells))
            .map(TableCell::measurement_scratch_bytes)
            .max()
            .unwrap_or(0)
    }

    fn column_widths(&self, column_count: usize, measurement_scratch: &mut String) -> Vec<usize> {
        let mut widths = vec![0; column_count];
        for (col_idx, cell) in self.header.cells.iter().enumerate() {
            widths[col_idx] = widths[col_idx].max(cell.width_with_scratch(measurement_scratch));
        }
        for row in &self.rows {
            for (col_idx, cell) in row.cells.iter().enumerate() {
                widths[col_idx] = widths[col_idx].max(cell.width_with_scratch(measurement_scratch));
            }
        }
        for width in &mut widths {
            *width = (*width).max(1);
        }
        widths
    }
}

fn push_stacked_label(lines: &mut Vec<Line<'_>>, row: usize) {
    if !lines.is_empty() {
        lines.push(Line::default());
    }
    lines.push(Line::from(if row == 0 {
        "Header".to_owned()
    } else {
        format!("Row {row}")
    }));
}

fn push_stacked_row<'a>(
    lines: &mut Vec<Line<'a>>,
    row: usize,
    cells: Vec<TableCell<'a>>,
    style: Style,
) {
    push_stacked_label(lines, row);
    for (index, cell) in cells.into_iter().enumerate() {
        let mut spans = vec![Span::styled(format!("[{}] ", index + 1), style)];
        spans.extend(cell.spans.into_iter().map(|span| span.patch_style(style)));
        lines.push(Line::from(spans));
    }
}

#[derive(Default)]
struct TableHeader<'a> {
    cells: Vec<TableCell<'a>>,
}

impl<'a> TableHeader<'a> {
    fn render<S: StyleSheet>(
        &self,
        column_widths: &[usize],
        alignments: &[Alignment],
        styles: &S,
        measurement_scratch: &mut String,
    ) -> Line<'a> {
        render_line(
            &self.cells,
            column_widths,
            alignments,
            styles.table_header(),
            styles.table_border(),
            measurement_scratch,
        )
    }
}

#[derive(Default)]
struct TableRow<'a> {
    cells: Vec<TableCell<'a>>,
}

impl<'a> TableRow<'a> {
    fn render<S: StyleSheet>(
        &self,
        column_widths: &[usize],
        alignments: &[Alignment],
        styles: &S,
        measurement_scratch: &mut String,
    ) -> Line<'a> {
        render_line(
            &self.cells,
            column_widths,
            alignments,
            styles.table_cell(),
            styles.table_border(),
            measurement_scratch,
        )
    }
}

#[derive(Default)]
struct TableCell<'a> {
    spans: Vec<Span<'a>>,
}

impl<'a> TableCell<'a> {
    fn push(&mut self, span: Span<'a>) {
        self.spans.push(span);
    }

    #[cfg(test)]
    fn width(&self) -> usize {
        let mut scratch = String::with_capacity(self.measurement_scratch_bytes());
        self.width_with_scratch(&mut scratch)
    }

    fn measurement_scratch_bytes(&self) -> usize {
        if self.spans.len() < 2 {
            return 0;
        }
        self.spans
            .iter()
            .fold(0, |bytes, span| bytes.saturating_add(span.content.len()))
    }

    fn width_with_scratch(&self, scratch: &mut String) -> usize {
        if let [span] = self.spans.as_slice() {
            return span.width();
        }
        scratch.clear();
        for span in &self.spans {
            scratch.push_str(&span.content);
        }
        scratch.width()
    }

    fn render_spans(
        &self,
        column_width: usize,
        content_width: usize,
        alignment: Alignment,
        style: Style,
    ) -> Vec<Span<'a>> {
        let (pad_left, pad_right) = padding(column_width, content_width, alignment);
        let mut spans = vec![Span::styled(" ".repeat(pad_left + 1), style)];

        for span in &self.spans {
            let mut span = span.clone();
            span.style = span.style.patch(style);
            spans.push(span);
        }
        spans.push(Span::styled(" ".repeat(pad_right + 1), style));
        spans
    }
}

#[derive(Clone, Copy)]
struct BorderGlyphs {
    left: char,
    intersection: char,
    right: char,
}

impl BorderGlyphs {
    const fn new(left: char, intersection: char, right: char) -> Self {
        Self {
            left,
            intersection,
            right,
        }
    }

    fn render<'a>(self, column_widths: &[usize], style: Style) -> Line<'a> {
        let mut border = String::new();
        border.push(self.left);
        for (index, width) in column_widths.iter().enumerate() {
            for _ in 0..(width + 2) {
                border.push(HORIZONTAL_BORDER);
            }
            if index + 1 < column_widths.len() {
                border.push(self.intersection);
            }
        }
        border.push(self.right);
        Line::from(Span::styled(border, style))
    }
}

fn render_line<'a>(
    cells: &[TableCell<'a>],
    column_widths: &[usize],
    alignments: &[Alignment],
    content_style: Style,
    border_style: Style,
    measurement_scratch: &mut String,
) -> Line<'a> {
    let mut spans = vec![Span::styled(VERTICAL_BORDER, border_style)];
    let empty_cell = TableCell::default();
    for (column_index, &column_width) in column_widths.iter().enumerate() {
        let cell = cells.get(column_index).unwrap_or(&empty_cell);
        let alignment = alignments
            .get(column_index)
            .copied()
            .unwrap_or(Alignment::None);
        let content_width = cell.width_with_scratch(measurement_scratch);
        spans.extend(cell.render_spans(column_width, content_width, alignment, content_style));
        spans.push(Span::styled(VERTICAL_BORDER, border_style));
    }
    Line::from(spans)
}

fn padding(column_width: usize, content_width: usize, alignment: Alignment) -> (usize, usize) {
    if content_width >= column_width {
        return (0, 0);
    }
    let total_pad = column_width - content_width;
    match alignment {
        Alignment::Left | Alignment::None => (0, total_pad),
        Alignment::Right => (total_pad, 0),
        Alignment::Center => {
            let left = total_pad / 2;
            (left, total_pad - left)
        }
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use itertools::Itertools;
    use pretty_assertions::assert_eq;
    use ratatui_core::style::{Style, Stylize};
    use ratatui_core::text::{Line, Span, Text};

    use super::*;
    use crate::{from_str, from_str_with_options, DefaultStyleSheet, Options, StyleSheet};

    #[test]
    fn empty_table() {
        let builder = TableBuilder::new(vec![]);
        assert!(builder.render(&DefaultStyleSheet).is_empty());
    }

    #[test]
    fn single_cell() {
        let mut builder = TableBuilder::new(vec![Alignment::None]);
        builder.start_cell();
        builder.push_span(Span::raw("hi"));
        builder.finish_cell();
        builder.finish_header();
        assert_eq!(builder.render(&DefaultStyleSheet).len(), 4);
    }

    #[test]
    fn ragged_rows_charge_and_preserve_implicit_empty_cells() {
        let mut builder = TableBuilder::new(vec![Alignment::None, Alignment::None]);
        builder.configure(
            LayoutOptions::default()
                .with_width(Some(80))
                .table_limits(TableLimits {
                    max_cells: 3,
                    max_buffer_bytes: 4 * 1024 * 1024,
                }),
            80,
            &DefaultStyleSheet,
        );
        for value in ["A", "B"] {
            builder.start_cell();
            builder.push_span(Span::raw(value));
            builder.finish_cell();
        }
        builder.finish_header();
        builder.start_cell();
        builder.push_span(Span::raw("x"));
        builder.finish_cell();
        builder.finish_row();

        let (lines, fallback) = builder.render_with_fallback(&DefaultStyleSheet);

        assert_eq!(fallback, Some(TableFallback::CellLimit));
        assert_eq!(
            lines.iter().map(ToString::to_string).collect_vec(),
            ["Header", "[1] A", "[2] B", "", "Row 1", "[1] x", "[2] "]
        );
    }

    #[test]
    fn measurement_scratch_budget_is_reused_between_cells() {
        let content = "x".repeat(2_100_000);
        let (left, right) = content.split_at(content.len() / 2);
        let mut builder = TableBuilder::new(vec![Alignment::None, Alignment::None]);
        builder.configure(
            LayoutOptions::default()
                .with_width(Some(u16::MAX))
                .table_limits(TableLimits {
                    max_cells: 2,
                    max_buffer_bytes: 4 * 1024 * 1024,
                }),
            usize::MAX,
            &DefaultStyleSheet,
        );
        for style in [Style::new().bold(), Style::new().italic()] {
            builder.start_cell();
            builder.push_span(Span::styled(left, style));
            builder.push_span(Span::styled(right, Style::default()));
            builder.finish_cell();
        }
        builder.finish_header();

        assert_eq!(
            builder.fallback, None,
            "one scratch buffer should be reused rather than charging both cells"
        );
        let (_, fallback) = builder.render_with_fallback(&DefaultStyleSheet);
        assert_eq!(fallback, None);
    }

    #[test]
    fn over_budget_measurement_scratch_falls_back_before_width_measurement() {
        let content = "x".repeat(5 * 1024 * 1024);
        let (left, right) = content.split_at(content.len() / 2);
        let mut builder = TableBuilder::new(vec![Alignment::None]);
        builder.configure(
            LayoutOptions::default()
                .with_width(Some(u16::MAX))
                .table_limits(TableLimits {
                    max_cells: 1,
                    max_buffer_bytes: 4 * 1024 * 1024,
                }),
            usize::MAX,
            &DefaultStyleSheet,
        );
        builder.start_cell();
        builder.push_span(Span::styled(left, Style::new().bold()));
        builder.push_span(Span::raw(right));
        builder.finish_cell();
        builder.finish_header();

        let (_, fallback) = builder.render_with_fallback(&DefaultStyleSheet);

        assert_eq!(fallback, Some(TableFallback::BufferLimit));
    }

    #[test]
    fn padding_for_each_alignment() {
        assert_eq!(padding(10, 3, Alignment::Left), (0, 7));
        assert_eq!(padding(10, 3, Alignment::Right), (7, 0));
        assert_eq!(padding(10, 4, Alignment::Center), (3, 3));
        assert_eq!(padding(10, 3, Alignment::Center), (3, 4));
    }

    #[test]
    fn cell_style_covers_padding_and_empty_cells() {
        let style = Style::new().on_green();
        let cell = TableCell {
            spans: vec![Span::raw("x")],
        };
        assert_eq!(
            cell.render_spans(4, 1, Alignment::Center, style),
            [
                Span::styled("  ", style),
                Span::styled("x", style),
                Span::styled("   ", style),
            ]
        );

        let empty_cell = TableCell::default();
        assert_eq!(
            empty_cell.render_spans(4, 0, Alignment::Right, style),
            [Span::styled("     ", style), Span::styled(" ", style)]
        );
    }

    #[test]
    fn column_widths_have_a_minimum_of_one() {
        let mut builder = TableBuilder::new(vec![]);
        builder.header.cells.push(TableCell::default());
        assert_eq!(builder.column_widths(1, &mut String::new()), vec![1]);
    }

    #[test]
    fn styled_cell_width() {
        let cell = TableCell {
            spans: vec![Span::from("hello").bold(), Span::raw(" world")],
        };
        assert_eq!(cell.width(), 11);
    }

    #[test]
    fn emoji_cell_width() {
        let cell = TableCell {
            spans: vec![Span::raw("✅"), Span::raw(" ok")],
        };
        assert_eq!(cell.width(), 5);
    }

    #[test]
    fn cjk_cell_width() {
        let cell = TableCell {
            spans: vec![Span::raw("日本"), Span::raw(" ok")],
        };
        assert_eq!(cell.width(), 7);
    }

    #[test]
    fn table_with_alignment() {
        let text = from_str(indoc! {"
                | Left | Center | Right |
                |:-----|:------:|------:|
                | a    | b      | c     |
            "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
        assert_eq!(
            rendered,
            [
                "┌──────┬────────┬───────┐",
                "│ Left │ Center │ Right │",
                "├──────┼────────┼───────┤",
                "│ a    │   b    │     c │",
                "└──────┴────────┴───────┘",
            ]
        );
    }

    #[test]
    fn table_without_outer_pipes() {
        let text = from_str(indoc! {"
            A | B
            ---|---
            a | b
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();

        assert_eq!(
            rendered,
            [
                "┌───┬───┐",
                "│ A │ B │",
                "├───┼───┤",
                "│ a │ b │",
                "└───┴───┘",
            ]
        );
    }

    #[test]
    fn escaped_pipe_stays_inside_its_cell() {
        let text = from_str(indoc! {"
            | Value |
            |-------|
            | a \\| b |
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();

        assert_eq!(
            rendered,
            [
                "┌───────┐",
                "│ Value │",
                "├───────┤",
                "│ a | b │",
                "└───────┘",
            ]
        );
    }

    #[test]
    fn table_with_cjk_content() {
        let text = from_str(indoc! {"
                | Latin | CJK |
                |-------|-----|
                | a     | 日本 |
            "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
        assert_eq!(
            rendered,
            [
                "┌───────┬──────┐",
                "│ Latin │ CJK  │",
                "├───────┼──────┤",
                "│ a     │ 日本 │",
                "└───────┴──────┘",
            ]
        );
        assert!(text.lines.iter().all(|line| line.width() == 16));
    }

    #[derive(Clone)]
    struct CustomTableStyleSheet;

    impl StyleSheet for CustomTableStyleSheet {
        fn heading(&self, _level: u8) -> Style {
            Style::default()
        }

        fn code(&self) -> Style {
            Style::default()
        }

        fn link(&self) -> Style {
            Style::new().blue().underlined()
        }

        fn blockquote(&self) -> Style {
            Style::default()
        }

        fn heading_meta(&self) -> Style {
            Style::default()
        }

        fn metadata_block(&self) -> Style {
            Style::default()
        }

        fn table_header(&self) -> Style {
            Style::new().on_blue()
        }

        fn table_cell(&self) -> Style {
            Style::new().red().on_green()
        }

        fn table_border(&self) -> Style {
            Style::new().red()
        }
    }

    #[test]
    fn custom_styles_apply_to_header_cells_body_cells_and_borders() {
        let border_style = Style::new().red();
        let header_style = Style::new().on_blue();
        let cell_style = Style::new().red().on_green();
        let options = Options::new(CustomTableStyleSheet);
        let text = from_str_with_options(
            indoc! {"
                | A |
                |---|
                | a |
            "},
            &options,
        );
        assert_eq!(
            text,
            Text::from_iter([
                Line::from(Span::styled("┌───┐", border_style)),
                Line::from_iter([
                    Span::styled("│", border_style),
                    Span::styled(" ", header_style),
                    Span::styled("A", header_style),
                    Span::styled(" ", header_style),
                    Span::styled("│", border_style),
                ]),
                Line::from(Span::styled("├───┤", border_style)),
                Line::from_iter([
                    Span::styled("│", border_style),
                    Span::styled(" ", cell_style),
                    Span::styled("a", cell_style),
                    Span::styled(" ", cell_style),
                    Span::styled("│", border_style),
                ]),
                Line::from(Span::styled("└───┘", border_style)),
            ])
        );
    }

    #[test]
    fn custom_cell_style_composes_with_inline_formatting() {
        let options = Options::new(CustomTableStyleSheet);
        let text = from_str_with_options(
            indoc! {"
                | A |
                |---|
                | **bold** |
            "},
            &options,
        );
        let body = &text.lines[3];

        assert!(body
            .spans
            .contains(&Span::styled("bold", Style::new().bold().red().on_green())));
    }

    #[test]
    fn table_cell_style_overrides_conflicting_inline_properties() {
        let options = Options::new(CustomTableStyleSheet);
        let text = from_str_with_options(
            indoc! {"
                | A |
                |---|
                | [docs](url) |
            "},
            &options,
        );
        let link = text.lines[3]
            .spans
            .iter()
            .find(|span| span.content == "docs")
            .expect("link cell content");

        assert_eq!(
            link,
            &Span::styled("docs", Style::new().red().underlined().on_green())
        );
    }

    #[test]
    fn table_preserves_surrounding_paragraph_spacing() {
        let text = from_str(indoc! {"
                Before

                | A |
                |---|
                | a |

                After
            "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
        assert_eq!(
            rendered,
            [
                "Before",
                "",
                "┌───┐",
                "│ A │",
                "├───┤",
                "│ a │",
                "└───┘",
                "",
                "After",
            ]
        );
    }

    #[test]
    fn consecutive_tables_keep_separate_layout_state() {
        let text = from_str(indoc! {"
            | Long |
            |------|
            | value |

            | A |
            |---|
            | b |
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();

        assert_eq!(
            rendered,
            [
                "┌───────┐",
                "│ Long  │",
                "├───────┤",
                "│ value │",
                "└───────┘",
                "",
                "┌───┐",
                "│ A │",
                "├───┤",
                "│ b │",
                "└───┘",
            ]
        );
    }

    #[test]
    fn empty_cells_keep_minimum_column_width() {
        let text = from_str(indoc! {"
            | A | B |
            |---|---|
            |   |   |
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
        assert_eq!(
            rendered,
            [
                "┌───┬───┐",
                "│ A │ B │",
                "├───┼───┤",
                "│   │   │",
                "└───┴───┘",
            ]
        );
    }

    #[test]
    fn header_only_table_has_a_complete_frame() {
        let text = from_str(indoc! {"
            | A |
            |---|
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();

        #[rustfmt::skip]
        let expected = [
            "┌───┐",
            "│ A │",
            "├───┤",
            "└───┘",
        ];
        assert_eq!(rendered, expected);
    }

    #[test]
    fn short_rows_are_padded_and_extra_cells_are_ignored() {
        let text = from_str(indoc! {"
            | A | B |
            |---|---|
            | one |
            | x | y | ignored |
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();

        assert_eq!(
            rendered,
            [
                "┌─────┬───┐",
                "│ A   │ B │",
                "├─────┼───┤",
                "│ one │   │",
                "│ x   │ y │",
                "└─────┴───┘",
            ]
        );
    }

    #[test]
    fn table_with_inline_code() {
        let text = from_str(indoc! {"
            | Name | Type |
            |------|------|
            | foo  | `u32` |
        "});
        let code_style = Style::new().white().on_black();
        let code = text.lines[3]
            .spans
            .iter()
            .find(|span| span.content == "u32")
            .expect("inline code cell content");
        assert_eq!(code, &Span::styled("u32", code_style));
    }

    #[test]
    fn table_with_bold_in_cells() {
        let text = from_str(indoc! {"
            | Col |
            |-----|
            | **bold** |
        "});
        let bold = text.lines[3]
            .spans
            .iter()
            .find(|span| span.content == "bold")
            .expect("bold cell content");
        assert_eq!(bold, &Span::styled("bold", Style::new().bold()));
    }

    #[test]
    fn table_keeps_link_destination_in_cell() {
        let text = from_str(indoc! {"
            | Link |
            |------|
            | [docs](u) |
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
        assert_eq!(
            rendered,
            [
                "┌──────────┐",
                "│ Link     │",
                "├──────────┤",
                "│ docs (u) │",
                "└──────────┘",
            ]
        );
        let link_style = DefaultStyleSheet.link();
        assert_eq!(
            text.lines[3],
            Line::from_iter([
                Span::styled("│", DefaultStyleSheet.table_border()),
                Span::raw(" "),
                Span::styled("docs", link_style),
                Span::raw(" ("),
                Span::styled("u", link_style),
                Span::raw(")"),
                Span::raw(" "),
                Span::styled("│", DefaultStyleSheet.table_border()),
            ])
        );
    }

    #[test]
    fn table_keeps_inline_features_in_cell() {
        let text = from_str(indoc! {"
            | Value |
            |-------|
            | <em>x</em> $y$ |
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
        assert_eq!(
            rendered,
            [
                "┌────────────────┐",
                "│ Value          │",
                "├────────────────┤",
                "│ <em>x</em> $y$ │",
                "└────────────────┘",
            ]
        );

        let row = &text.lines[3];
        assert!(row
            .spans
            .contains(&Span::styled("<em>", DefaultStyleSheet.html())));
        assert!(row
            .spans
            .contains(&Span::styled("$y$", DefaultStyleSheet.math_inline())));
    }

    #[test]
    fn table_routes_inline_content_through_the_active_cell() {
        let text = from_str(indoc! {"
            | Value |
            |-------|
            | **bold** `code` [link](url) <em>x</em> $y$ [^n] |

            [^n]: note
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();

        assert_eq!(rendered[3], "│ bold code link (url) <em>x</em> $y$ [n] │");
    }

    #[test]
    fn block_markers_inside_cells_remain_inline_text() {
        let text = from_str(indoc! {"
            | Value |
            |-------|
            | # heading |
            | > quote |
            | - list |
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();

        assert_eq!(
            rendered,
            [
                "┌───────────┐",
                "│ Value     │",
                "├───────────┤",
                "│ # heading │",
                "│ > quote   │",
                "│ - list    │",
                "└───────────┘",
            ]
        );
    }

    #[test]
    fn table_in_blockquote_keeps_quote_prefix() {
        let text = from_str(indoc! {"
            > | A |
            > |---|
            > | a |
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
        #[rustfmt::skip]
        let expected = [
            "> ┌───┐",
            "> │ A │",
            "> ├───┤",
            "> │ a │",
            "> └───┘",
        ];
        assert_eq!(rendered, expected);
    }

    #[test]
    fn table_list_item_keeps_marker_and_continuation_indent() {
        let text = from_str(indoc! {"
            - | A |
              |---|
              | a |
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
        #[rustfmt::skip]
        let expected = [
            "- ┌───┐",
            "  │ A │",
            "  ├───┤",
            "  │ a │",
            "  └───┘",
        ];
        assert_eq!(rendered, expected);
    }

    #[test]
    fn later_table_in_list_uses_continuation_indent() {
        let text = from_str(indoc! {"
            - | A |
              |---|
              | a |

              | B |
              |---|
              | b |
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
        assert_eq!(
            rendered,
            [
                "- ┌───┐",
                "  │ A │",
                "  ├───┤",
                "  │ a │",
                "  └───┘",
                "",
                "  ┌───┐",
                "  │ B │",
                "  ├───┤",
                "  │ b │",
                "  └───┘",
            ]
        );
    }

    #[test]
    fn ordered_table_list_item_uses_full_marker_width() {
        let text = from_str(indoc! {"
            10. | A |
                |---|
                | a |
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
        assert_eq!(
            rendered,
            [
                "10. ┌───┐",
                "    │ A │",
                "    ├───┤",
                "    │ a │",
                "    └───┘",
            ]
        );
    }

    #[test]
    fn nested_table_list_item_uses_nested_marker_width() {
        let text = from_str(indoc! {"
            - Parent
              - | A |
                |---|
                | a |
        "});
        let rendered = text.lines.iter().map(ToString::to_string).collect_vec();
        assert_eq!(
            rendered,
            [
                "- Parent",
                "    - ┌───┐",
                "      │ A │",
                "      ├───┤",
                "      │ a │",
                "      └───┘",
            ]
        );
    }

    #[test]
    fn table_snapshot() {
        let text = from_str(indoc! {"
                | Name | Value |
                |------|-------|
                | foo  | bar   |
                | baz  | qux   |
            "});
        insta::assert_snapshot!(text);
    }
}
