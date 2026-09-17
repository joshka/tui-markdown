//! Wrap text and fit tables to a supplied width for both batch and streaming output.

use std::fmt;

use ratatui_core::text::{Line, Span, Text};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Limit the buffers used to build table grids with [`crate::Options::table_limits`].
///
/// If a table grid exceeds a limit, the renderer lists cells vertically with numbers instead.
/// Cell content is kept. Defaults are 8,192 logical cells and 4 MiB of tracked buffer capacity.
///
/// These limits apply only when [`crate::Options::width`] is set. They do not cap source storage,
/// rendered output, parser allocations, or total memory use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TableLimits {
    /// Maximum number of buffered grid cells, including header cells and implicit empty cells.
    pub max_cells: usize,
    /// Maximum tracked capacity in bytes for grid-presentation buffers.
    pub max_buffer_bytes: usize,
}

impl Default for TableLimits {
    fn default() -> Self {
        Self {
            max_cells: 8_192,
            max_buffer_bytes: 4 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LayoutOptions {
    width: Option<u16>,
    table_limits: TableLimits,
    wide_grapheme_replacement: char,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            width: None,
            table_limits: TableLimits::default(),
            wide_grapheme_replacement: '-',
        }
    }
}

impl LayoutOptions {
    pub(crate) const fn with_width(mut self, width: Option<u16>) -> Self {
        self.width = width;
        self
    }

    pub(crate) const fn width(self) -> Option<u16> {
        self.width
    }

    pub(crate) const fn table_limits(mut self, limits: TableLimits) -> Self {
        self.table_limits = limits;
        self
    }

    pub(crate) const fn limits(self) -> TableLimits {
        self.table_limits
    }

    pub(crate) fn with_wide_grapheme_replacement(
        mut self,
        replacement: char,
    ) -> Result<Self, InvalidReplacementCharacter> {
        if !replacement.is_ascii() || replacement.is_ascii_control() {
            return Err(InvalidReplacementCharacter);
        }
        self.wide_grapheme_replacement = replacement;
        Ok(self)
    }

    pub(crate) const fn wide_grapheme_replacement(self) -> char {
        self.wide_grapheme_replacement
    }
}

/// A replacement character was rejected because it was not printable ASCII.
///
/// Use a character from `U+0020` through `U+007E`, such as `-` or `*`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidReplacementCharacter;

impl fmt::Display for InvalidReplacementCharacter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("wide-grapheme replacement must be a printable ASCII character")
    }
}

impl std::error::Error for InvalidReplacementCharacter {}

pub(crate) fn wrap_text(text: Text<'_>, context: LayoutOptions) -> Text<'_> {
    wrap_text_with_checkpoint(text, context, usize::MAX).0
}

pub(crate) fn wrap_text_with_checkpoint(
    mut text: Text<'_>,
    context: LayoutOptions,
    checkpoint_row: usize,
) -> (Text<'_>, usize) {
    let Some(width) = context.width() else {
        let checkpoint = checkpoint_row.min(text.lines.len());
        return (text, checkpoint);
    };
    if width == 0 {
        text.lines.clear();
        return (text, 0);
    }
    let semantic_rows = text.lines.len();
    let mut lines = Vec::new();
    let mut display_checkpoint = 0;
    for (row, line) in text.lines.into_iter().enumerate() {
        if row == checkpoint_row {
            display_checkpoint = lines.len();
        }
        lines.extend(wrap_line(line, width, context.wide_grapheme_replacement()));
    }
    if checkpoint_row >= semantic_rows {
        display_checkpoint = lines.len();
    }
    text.lines = lines;
    (text, display_checkpoint)
}

fn wrap_line(line: Line<'_>, width: u16, replacement: char) -> Vec<Line<'_>> {
    let width = usize::from(width);
    let content: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    if line.width() <= width {
        let mut boundaries = line
            .spans
            .iter()
            .scan(0, |end, span| {
                *end += span.content.len();
                Some(*end)
            })
            .peekable();
        let crosses_style_boundary = content.grapheme_indices(true).any(|(start, grapheme)| {
            while boundaries.peek().is_some_and(|end| *end <= start) {
                boundaries.next();
            }
            boundaries
                .peek()
                .is_some_and(|end| *end < start + grapheme.len())
        });
        if !crosses_style_boundary {
            return vec![line];
        }
    }
    let mut output = Vec::new();
    let mut current = Line::default().style(line.style);
    current.alignment = line.alignment;
    let mut columns = 0;
    let mut spans = line.spans.iter();
    let mut active = spans.next();
    let mut span_end = active.map_or(0, |span| span.content.len());
    let mut previous_was_whitespace = true;
    let mut whitespace_token_fits = false;
    let mut word_boundaries = content.split_word_bound_indices();
    let mut word_boundary = word_boundaries.next();
    let mut replacement_buffer = [0; 4];
    let replacement = replacement.encode_utf8(&mut replacement_buffer);

    for (start, grapheme) in content.grapheme_indices(true) {
        let is_whitespace = grapheme.chars().all(char::is_whitespace);
        while word_boundary
            .as_ref()
            .is_some_and(|(segment_start, segment)| segment_start + segment.len() <= start)
        {
            word_boundary = word_boundaries.next();
        }
        if !is_whitespace && previous_was_whitespace {
            let token_width = content[start..]
                .graphemes(true)
                .take_while(|candidate| !candidate.chars().all(char::is_whitespace))
                .map(UnicodeWidthStr::width)
                .sum::<usize>();
            whitespace_token_fits = token_width <= width;
            if columns > 0 && whitespace_token_fits && columns + token_width > width {
                output.push(current);
                current = Line::default().style(line.style);
                current.alignment = line.alignment;
                columns = 0;
            }
        } else if !is_whitespace
            && !whitespace_token_fits
            && columns > 0
            && word_boundary
                .as_ref()
                .is_some_and(|(segment_start, segment)| {
                    *segment_start == start
                        && !segment.chars().all(char::is_whitespace)
                        && segment.width() <= width
                        && columns + segment.width() > width
                })
        {
            output.push(current);
            current = Line::default().style(line.style);
            current.alignment = line.alignment;
            columns = 0;
        }
        while start >= span_end {
            active = spans.next();
            span_end += active.map_or(0, |span| span.content.len());
            if active.is_none() {
                break;
            }
        }
        let (display, cells) = if grapheme.width() > width {
            (&*replacement, 1)
        } else {
            (grapheme, grapheme.width())
        };
        if columns > 0 && columns + cells > width {
            output.push(current);
            current = Line::default().style(line.style);
            current.alignment = line.alignment;
            columns = 0;
        }
        let style = active.map(|span| span.style).unwrap_or_default();
        if let Some(last) = current.spans.last_mut().filter(|span| span.style == style) {
            last.content.to_mut().push_str(display);
        } else {
            current.spans.push(Span::styled(display.to_owned(), style));
        }
        columns += cells;
        previous_was_whitespace = is_whitespace;
        if is_whitespace {
            whitespace_token_fits = false;
        }
    }
    output.push(current);
    output
}

#[cfg(test)]
mod tests {
    use ratatui_core::text::Text;

    use super::*;

    #[test]
    fn checkpoint_at_semantic_end_maps_after_all_wrapped_rows() {
        let text = Text::from("abcd");

        let (wrapped, checkpoint) =
            wrap_text_with_checkpoint(text, LayoutOptions::default().with_width(Some(2)), 1);

        assert_eq!(wrapped.lines.len(), 2);
        assert_eq!(checkpoint, 2);
    }
}
