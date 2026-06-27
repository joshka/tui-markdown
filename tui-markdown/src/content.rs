//! Rich markdown content model supporting mixed text and image blocks.
//!
//! The [`MarkdownContent`] type is returned by [`crate::parse()`] and represents a sequence of
//! renderable blocks. Unlike the simpler [`ratatui_core::text::Text`] returned by
//! [`crate::from_str()`], it can represent images as separate blocks that consumers can render
//! using terminal image protocols (when the `terminal-images` feature is enabled) or custom
//! rendering logic.

use ratatui_core::text::Text;

/// A single renderable block within a parsed markdown document.
#[derive(Debug, Clone)]
pub enum MarkdownBlock<'a> {
    /// Regular text content (headings, paragraphs, lists, code blocks, etc.).
    Text(Text<'a>),
    /// An image that can be rendered inline when the terminal supports it.
    Image {
        /// The image URL or file path.
        url: String,
        /// The alt text for the image (empty string if none provided).
        alt: String,
        /// Optional title from the markdown image syntax.
        title: Option<String>,
    },
}

/// A sequence of renderable markdown blocks.
///
/// Returned by [`crate::parse()`] and [`crate::parse_with_options()`]. Supports mixed content
/// where images are represented as separate blocks rather than being flattened to alt text.
///
/// # Example
///
/// ```
/// use tui_markdown::parse;
///
/// let content = parse("Hello\n\n![photo](pic.png)\n\nWorld");
/// for block in &content.blocks {
///     match block {
///         tui_markdown::MarkdownBlock::Text(text) => {
///             // Render text normally with ratatui
///         }
///         tui_markdown::MarkdownBlock::Image { url, alt, .. } => {
///             // Render image using terminal image protocol or fallback
///         }
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct MarkdownContent<'a> {
    /// The ordered sequence of blocks in this document.
    pub blocks: Vec<MarkdownBlock<'a>>,
}

impl<'a> MarkdownContent<'a> {
    /// Flatten all blocks into a single [`Text`], rendering images as alt-text fallback.
    ///
    /// This is equivalent to what [`crate::from_str()`] produces.
    pub fn into_text(self) -> Text<'a> {
        use ratatui_core::style::Style;
        use ratatui_core::text::{Line, Span};

        let mut lines: Vec<Line<'a>> = Vec::new();
        for block in self.blocks {
            match block {
                MarkdownBlock::Text(text) => {
                    lines.extend(text.lines);
                }
                MarkdownBlock::Image { alt, url, .. } => {
                    let style = Style::new().dim().italic();
                    let display = if alt.is_empty() {
                        format!("[img] {url}")
                    } else {
                        format!("[img] {alt}")
                    };
                    lines.push(Line::from(Span::styled(display, style)));
                }
            }
        }
        Text::from(lines)
    }
}
