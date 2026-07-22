//! Structured Markdown content with separate text and image blocks.
//!
//! [`MarkdownContent`] is returned by [`crate::parse`] and preserves images as distinct blocks
//! that consumers can render with their own image handling.

use ratatui_core::text::Text;

/// A single renderable block within a parsed Markdown document.
#[derive(Debug, Clone)]
pub enum MarkdownBlock<'a> {
    /// Regular text content such as headings, paragraphs, lists, and code blocks.
    Text(Text<'a>),
    /// An image that can be rendered separately from surrounding text.
    Image {
        /// The image URL or file path.
        url: String,
        /// The image alt text, or an empty string when none was provided.
        alt: String,
        /// The optional title from the Markdown image syntax.
        title: Option<String>,
    },
}

/// An ordered sequence of renderable Markdown blocks.
///
/// Returned by [`crate::parse`] and [`crate::parse_with_options`]. Images are represented as
/// separate blocks rather than being flattened to alt text.
///
/// # Example
///
/// ```
/// use tui_markdown::{parse, MarkdownBlock};
///
/// let content = parse("Hello\n\n![photo](pic.png)\n\nWorld");
/// for block in &content.blocks {
///     match block {
///         MarkdownBlock::Text(text) => {
///             // Render text normally with Ratatui.
///         }
///         MarkdownBlock::Image { url, alt, .. } => {
///             // Render the image with custom image handling.
///         }
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct MarkdownContent<'a> {
    /// The ordered sequence of blocks in this document.
    pub blocks: Vec<MarkdownBlock<'a>>,
}
