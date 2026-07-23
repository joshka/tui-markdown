//! Table rendering support for tui-markdown.
//!
//! A table must be buffered before rendering because every cell can increase its column's terminal
//! display width. [`TableBuilder`] collects the header and body rows, then renders their content,
//! alignment, padding, and Unicode box-drawing borders once pulldown-cmark closes the table.
//!
//! The parent renderer owns Markdown event handling and inline styles. This module owns only the
//! layout of a complete table.

use pulldown_cmark::Alignment;
use ratatui_core::style::Style;
use ratatui_core::text::{Line, Span};

use crate::StyleSheet;

const HORIZONTAL_BORDER: char = '─';
const VERTICAL_BORDER: &str = "│";
const TOP_BORDER: BorderGlyphs = BorderGlyphs::new('┌', '┬', '┐');
const HEADER_SEPARATOR: BorderGlyphs = BorderGlyphs::new('├', '┼', '┤');
const BOTTOM_BORDER: BorderGlyphs = BorderGlyphs::new('└', '┴', '┘');

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
}

impl<'a> TableBuilder<'a> {
    pub fn new(alignments: Vec<Alignment>) -> Self {
        Self {
            alignments,
            header: TableHeader::default(),
            rows: Vec::new(),
            current_row: TableRow::default(),
            current_cell: TableCell::default(),
        }
    }

    pub fn start_cell(&mut self) {
        self.current_cell = TableCell::default();
    }

    pub fn push_span(&mut self, span: Span<'a>) {
        self.current_cell.push(span);
    }

    pub fn finish_cell(&mut self) {
        let cell = std::mem::take(&mut self.current_cell);
        self.current_row.cells.push(cell);
    }

    pub fn finish_header(&mut self) {
        self.header.cells = std::mem::take(&mut self.current_row.cells);
    }

    pub fn finish_row(&mut self) {
        self.rows.push(std::mem::take(&mut self.current_row));
    }

    pub fn render<S: StyleSheet>(self, styles: &S) -> Vec<Line<'a>> {
        let column_count = self.column_count();
        if column_count == 0 {
            return Vec::new();
        }

        let column_widths = self.column_widths(column_count);
        let border_style = styles.table_border();

        let top_border = TOP_BORDER.render(&column_widths, border_style);
        let header = self.header.render(&column_widths, &self.alignments, styles);
        let header_separator = HEADER_SEPARATOR.render(&column_widths, border_style);
        let body = self
            .rows
            .iter()
            .map(|row| row.render(&column_widths, &self.alignments, styles));
        let bottom_border = BOTTOM_BORDER.render(&column_widths, border_style);

        let mut lines = vec![top_border, header, header_separator];
        lines.extend(body);
        lines.push(bottom_border);
        lines
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

    fn column_widths(&self, column_count: usize) -> Vec<usize> {
        let mut widths = vec![0; column_count];
        for (col_idx, cell) in self.header.cells.iter().enumerate() {
            widths[col_idx] = widths[col_idx].max(cell.width());
        }
        for row in &self.rows {
            for (col_idx, cell) in row.cells.iter().enumerate() {
                widths[col_idx] = widths[col_idx].max(cell.width());
            }
        }
        for width in &mut widths {
            *width = (*width).max(1);
        }
        widths
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
    ) -> Line<'a> {
        render_line(
            &self.cells,
            column_widths,
            alignments,
            styles.table_header(),
            styles.table_border(),
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
    ) -> Line<'a> {
        render_line(
            &self.cells,
            column_widths,
            alignments,
            styles.table_cell(),
            styles.table_border(),
        )
    }
}

fn render_line<'a>(
    cells: &[TableCell<'a>],
    column_widths: &[usize],
    alignments: &[Alignment],
    content_style: Style,
    border_style: Style,
) -> Line<'a> {
    let mut spans = vec![Span::styled(VERTICAL_BORDER, border_style)];
    let empty_cell = TableCell::default();
    for (column_index, &column_width) in column_widths.iter().enumerate() {
        let cell = cells.get(column_index).unwrap_or(&empty_cell);
        let alignment = alignments
            .get(column_index)
            .copied()
            .unwrap_or(Alignment::None);
        spans.extend(cell.render_spans(column_width, alignment, content_style));
        spans.push(Span::styled(VERTICAL_BORDER, border_style));
    }
    Line::from(spans)
}

#[derive(Default)]
struct TableCell<'a> {
    spans: Vec<Span<'a>>,
}

impl<'a> TableCell<'a> {
    fn push(&mut self, span: Span<'a>) {
        self.spans.push(span);
    }

    fn width(&self) -> usize {
        self.spans.iter().map(Span::width).sum()
    }

    fn render_spans(
        &self,
        column_width: usize,
        alignment: Alignment,
        style: Style,
    ) -> Vec<Span<'a>> {
        let (pad_left, pad_right) = padding(column_width, self.width(), alignment);
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
mod tests;
