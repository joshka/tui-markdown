//! Table rendering support for tui-markdown.
//!
//! This module provides a [`TableBuilder`] that accumulates table cells during pulldown-cmark
//! parsing and renders the complete table as a vector of [`Line`]s using Unicode box-drawing
//! characters for borders.

use pulldown_cmark::Alignment;
use ratatui_core::style::Style;
use ratatui_core::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::StyleSheet;

/// Accumulates table rows and cells during markdown parsing and renders the finished table.
pub(crate) struct TableBuilder<'a> {
    /// Column alignments from the GFM table header separator row.
    alignments: Vec<Alignment>,
    /// All rows (header + body). Each row is a Vec of cells; each cell is a Vec of Spans.
    rows: Vec<Vec<Vec<Span<'a>>>>,
    /// The cell currently being built.
    current_cell: Vec<Span<'a>>,
    /// Whether the first row (header) has been finished.
    header_finished: bool,
}

impl<'a> TableBuilder<'a> {
    /// Create a new builder with the given column alignments.
    pub(crate) fn new(alignments: Vec<Alignment>) -> Self {
        Self {
            alignments,
            rows: Vec::new(),
            current_cell: Vec::new(),
            header_finished: false,
        }
    }

    /// Start a new row (called for both TableHead and TableRow start tags).
    pub(crate) fn start_row(&mut self) {
        self.rows.push(Vec::new());
    }

    /// Finish the current row.
    pub(crate) fn finish_row(&mut self) {
        // Nothing extra needed -- cells are already pushed into the current row.
    }

    /// Mark the header as finished.
    pub(crate) fn finish_header(&mut self) {
        self.header_finished = true;
    }

    /// Start a new cell.
    pub(crate) fn start_cell(&mut self) {
        self.current_cell = Vec::new();
    }

    /// Push a span into the cell currently being built.
    pub(crate) fn push_span(&mut self, span: Span<'a>) {
        self.current_cell.push(span);
    }

    /// Finish the current cell and add it to the current row.
    pub(crate) fn finish_cell(&mut self) {
        let cell = std::mem::take(&mut self.current_cell);
        if let Some(row) = self.rows.last_mut() {
            row.push(cell);
        }
    }

    /// Render the completed table into a vector of [`Line`]s.
    pub(crate) fn render<S: StyleSheet>(self, styles: &S) -> Vec<Line<'a>> {
        if self.rows.is_empty() {
            return Vec::new();
        }

        let num_cols = self
            .alignments
            .len()
            .max(self.rows.iter().map(|r| r.len()).max().unwrap_or(0));
        if num_cols == 0 {
            return Vec::new();
        }

        // Calculate the display width of each cell's content.
        let col_widths = Self::compute_column_widths(&self.rows, num_cols);

        let border_style = styles.table_border();
        let header_style = styles.table_header();

        let mut lines: Vec<Line<'a>> = Vec::new();

        // Top border: ┌──────┬───────┐
        lines.push(Self::horizontal_border(
            &col_widths,
            '┌',
            '┬',
            '┐',
            border_style,
        ));

        for (row_idx, row) in self.rows.iter().enumerate() {
            // Data row: │ content │ content │
            let is_header = row_idx == 0;
            let cell_style = if is_header {
                header_style
            } else {
                Style::default()
            };

            let mut spans: Vec<Span<'a>> = Vec::new();
            spans.push(Span::styled("│", border_style));

            for (col_idx, &col_width) in col_widths.iter().enumerate() {
                spans.push(Span::raw(" "));

                let cell_spans = row.get(col_idx);
                let content_width = cell_spans.map(|s| Self::spans_width(s)).unwrap_or(0);
                let alignment = self
                    .alignments
                    .get(col_idx)
                    .copied()
                    .unwrap_or(Alignment::None);

                let (pad_left, pad_right) = Self::padding(col_width, content_width, alignment);

                if pad_left > 0 {
                    spans.push(Span::raw(" ".repeat(pad_left)));
                }

                if let Some(cell_content) = cell_spans {
                    for span in cell_content {
                        let mut styled_span = span.clone();
                        if is_header {
                            styled_span.style = styled_span.style.patch(cell_style);
                        }
                        spans.push(styled_span);
                    }
                }

                if pad_right > 0 {
                    spans.push(Span::raw(" ".repeat(pad_right)));
                }

                spans.push(Span::raw(" "));
                spans.push(Span::styled("│", border_style));
            }

            lines.push(Line::from(spans));

            // After header row, insert separator: ├──────┼───────┤
            if is_header {
                lines.push(Self::horizontal_border(
                    &col_widths,
                    '├',
                    '┼',
                    '┤',
                    border_style,
                ));
            }
        }

        // Bottom border: └──────┴───────┘
        lines.push(Self::horizontal_border(
            &col_widths,
            '└',
            '┴',
            '┘',
            border_style,
        ));

        lines
    }

    /// Compute the maximum display width for each column.
    fn compute_column_widths(rows: &[Vec<Vec<Span<'_>>>], num_cols: usize) -> Vec<usize> {
        let mut widths = vec![0_usize; num_cols];
        for row in rows {
            for (col_idx, cell) in row.iter().enumerate() {
                if col_idx < num_cols {
                    let w = Self::spans_width(cell);
                    if w > widths[col_idx] {
                        widths[col_idx] = w;
                    }
                }
            }
        }
        // Ensure a minimum column width of 1 so empty columns still render.
        for w in &mut widths {
            if *w == 0 {
                *w = 1;
            }
        }
        widths
    }

    /// Calculate the display width of a slice of spans using Unicode column widths.
    fn spans_width(spans: &[Span<'_>]) -> usize {
        spans.iter().map(|s| s.content.width()).sum()
    }

    /// Calculate left and right padding for a cell given column width, content width, and alignment.
    fn padding(col_width: usize, content_width: usize, alignment: Alignment) -> (usize, usize) {
        if content_width >= col_width {
            return (0, 0);
        }
        let total_pad = col_width - content_width;
        match alignment {
            Alignment::Left | Alignment::None => (0, total_pad),
            Alignment::Right => (total_pad, 0),
            Alignment::Center => {
                let left = total_pad / 2;
                let right = total_pad - left;
                (left, right)
            }
        }
    }

    /// Build a horizontal border line (top, separator, or bottom).
    fn horizontal_border<'b>(
        col_widths: &[usize],
        left: char,
        mid: char,
        right: char,
        style: Style,
    ) -> Line<'b> {
        let mut s = String::new();
        s.push(left);
        for (i, &w) in col_widths.iter().enumerate() {
            // Each cell has 1 space padding on each side, so the dash segment is w + 2.
            for _ in 0..(w + 2) {
                s.push('─');
            }
            if i < col_widths.len() - 1 {
                s.push(mid);
            }
        }
        s.push(right);
        Line::from(Span::styled(s, style))
    }
}

#[cfg(test)]
mod tests {
    use pulldown_cmark::Alignment;
    use ratatui_core::style::Style;

    use super::*;
    use crate::DefaultStyleSheet;

    #[test]
    fn empty_table() {
        let builder = TableBuilder::new(vec![]);
        let lines = builder.render(&DefaultStyleSheet);
        assert!(lines.is_empty());
    }

    #[test]
    fn single_cell() {
        let mut builder = TableBuilder::new(vec![Alignment::None]);
        builder.start_row();
        builder.start_cell();
        builder.push_span(Span::raw("hi"));
        builder.finish_cell();
        builder.finish_row();
        builder.finish_header();

        let lines = builder.render(&DefaultStyleSheet);
        // Should have: top border, header row, separator, bottom border (no body rows).
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn padding_left() {
        let (l, r) = TableBuilder::padding(10, 3, Alignment::Left);
        assert_eq!((l, r), (0, 7));
    }

    #[test]
    fn padding_right() {
        let (l, r) = TableBuilder::padding(10, 3, Alignment::Right);
        assert_eq!((l, r), (7, 0));
    }

    #[test]
    fn padding_center() {
        let (l, r) = TableBuilder::padding(10, 4, Alignment::Center);
        assert_eq!((l, r), (3, 3));
    }

    #[test]
    fn padding_center_odd() {
        let (l, r) = TableBuilder::padding(10, 3, Alignment::Center);
        // 7 total pad, left=3, right=4
        assert_eq!((l, r), (3, 4));
    }

    #[test]
    fn col_widths_minimum_one() {
        let rows: Vec<Vec<Vec<Span<'_>>>> = vec![vec![vec![]]];
        let widths = TableBuilder::compute_column_widths(&rows, 1);
        assert_eq!(widths, vec![1]);
    }

    #[test]
    fn styled_spans_width() {
        let spans = vec![
            Span::styled("hello", Style::new().bold()),
            Span::raw(" world"),
        ];
        assert_eq!(TableBuilder::spans_width(&spans), 11);
    }

    #[test]
    fn emoji_spans_width() {
        // ✅ is 2 display columns (not 3 bytes), 🟧 is also 2 display columns (not 4 bytes).
        let spans = vec![Span::raw("✅"), Span::raw(" ok")];
        assert_eq!(TableBuilder::spans_width(&spans), 5); // 2 + 3
    }
}
