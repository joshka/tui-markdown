//! Table rendering support for tui-markdown.
//!
//! This module accumulates table cells during parsing and renders complete tables with Unicode
//! box-drawing borders.

use pulldown_cmark::Alignment;
use ratatui_core::style::Style;
use ratatui_core::text::{Line, Span};

use crate::StyleSheet;

/// Accumulates table rows and cells during Markdown parsing.
pub(crate) struct TableBuilder<'a> {
    alignments: Vec<Alignment>,
    rows: Vec<Vec<Vec<Span<'a>>>>,
    current_cell: Vec<Span<'a>>,
}

impl<'a> TableBuilder<'a> {
    pub(crate) fn new(alignments: Vec<Alignment>) -> Self {
        Self {
            alignments,
            rows: Vec::new(),
            current_cell: Vec::new(),
        }
    }

    pub(crate) fn start_row(&mut self) {
        self.rows.push(Vec::new());
    }

    pub(crate) fn start_cell(&mut self) {
        self.current_cell = Vec::new();
    }

    pub(crate) fn push_span(&mut self, span: Span<'a>) {
        self.current_cell.push(span);
    }

    pub(crate) fn finish_cell(&mut self) {
        let cell = std::mem::take(&mut self.current_cell);
        if let Some(row) = self.rows.last_mut() {
            row.push(cell);
        }
    }

    pub(crate) fn render<S: StyleSheet>(self, styles: &S) -> Vec<Line<'a>> {
        if self.rows.is_empty() {
            return Vec::new();
        }

        let num_cols = self
            .alignments
            .len()
            .max(self.rows.iter().map(Vec::len).max().unwrap_or(0));
        if num_cols == 0 {
            return Vec::new();
        }

        let col_widths = Self::compute_column_widths(&self.rows, num_cols);
        let border_style = styles.table_border();
        let header_style = styles.table_header();
        let mut lines = Vec::new();

        lines.push(Self::horizontal_border(
            &col_widths,
            '┌',
            '┬',
            '┐',
            border_style,
        ));

        for (row_idx, row) in self.rows.iter().enumerate() {
            let is_header = row_idx == 0;
            let mut spans = vec![Span::styled("│", border_style)];

            for (col_idx, &col_width) in col_widths.iter().enumerate() {
                spans.push(Span::raw(" "));
                let cell_spans = row.get(col_idx);
                let content_width = cell_spans
                    .map(|spans| Self::spans_width(spans))
                    .unwrap_or(0);
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
                            styled_span.style = styled_span.style.patch(header_style);
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

        lines.push(Self::horizontal_border(
            &col_widths,
            '└',
            '┴',
            '┘',
            border_style,
        ));
        lines
    }

    fn compute_column_widths(rows: &[Vec<Vec<Span<'_>>>], num_cols: usize) -> Vec<usize> {
        let mut widths = vec![0; num_cols];
        for row in rows {
            for (col_idx, cell) in row.iter().enumerate() {
                if col_idx < num_cols {
                    let width = Self::spans_width(cell);
                    if width > widths[col_idx] {
                        widths[col_idx] = width;
                    }
                }
            }
        }
        for width in &mut widths {
            if *width == 0 {
                *width = 1;
            }
        }
        widths
    }

    /// Calculate the display width of a slice of spans using Unicode column widths.
    fn spans_width(spans: &[Span<'_>]) -> usize {
        spans.iter().map(Span::width).sum()
    }

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
                (left, total_pad - left)
            }
        }
    }

    fn horizontal_border<'b>(
        col_widths: &[usize],
        left: char,
        mid: char,
        right: char,
        style: Style,
    ) -> Line<'b> {
        let mut border = String::new();
        border.push(left);
        for (index, width) in col_widths.iter().enumerate() {
            for _ in 0..(width + 2) {
                border.push('─');
            }
            if index + 1 < col_widths.len() {
                border.push(mid);
            }
        }
        border.push(right);
        Line::from(Span::styled(border, style))
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::style::Stylize;

    use super::*;
    use crate::DefaultStyleSheet;

    #[test]
    fn empty_table() {
        let builder = TableBuilder::new(vec![]);
        assert!(builder.render(&DefaultStyleSheet).is_empty());
    }

    #[test]
    fn single_cell() {
        let mut builder = TableBuilder::new(vec![Alignment::None]);
        builder.start_row();
        builder.start_cell();
        builder.push_span(Span::raw("hi"));
        builder.finish_cell();
        assert_eq!(builder.render(&DefaultStyleSheet).len(), 4);
    }

    #[test]
    fn padding() {
        assert_eq!(TableBuilder::padding(10, 3, Alignment::Left), (0, 7));
        assert_eq!(TableBuilder::padding(10, 3, Alignment::Right), (7, 0));
        assert_eq!(TableBuilder::padding(10, 4, Alignment::Center), (3, 3));
        assert_eq!(TableBuilder::padding(10, 3, Alignment::Center), (3, 4));
    }

    #[test]
    fn col_widths_minimum_one() {
        let rows = vec![vec![vec![]]];
        assert_eq!(TableBuilder::compute_column_widths(&rows, 1), vec![1]);
    }

    #[test]
    fn styled_spans_width() {
        let spans = vec![Span::from("hello").bold(), Span::raw(" world")];
        assert_eq!(TableBuilder::spans_width(&spans), 11);
    }

    #[test]
    fn emoji_spans_width() {
        let spans = vec![Span::raw("✅"), Span::raw(" ok")];
        assert_eq!(TableBuilder::spans_width(&spans), 5);
    }

    #[test]
    fn cjk_spans_width() {
        let spans = vec![Span::raw("日本"), Span::raw(" ok")];
        assert_eq!(TableBuilder::spans_width(&spans), 7);
    }
}
