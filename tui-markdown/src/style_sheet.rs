//! Style sheet abstraction for tui-markdown.
//!
//! The library used to hard–code all color and attribute choices in an internal `styles` module.
//! That made it impossible for downstream crates to provide their own look-and-feel. The
//! [`StyleSheet`] trait makes it possible to customize the styles used to display the
//! [`ratatui_core::style::Style`] values the renderer needs.
//!
//! Users that are happy with the stock colors do not have to do anything – the crate exports a
//! [`DefaultStyleSheet`] which matches the old behaviour and is used by default. Projects that want
//! to theme the output can implement the trait for their own type and pass an instance via
//! [`crate::Options`].

use ratatui_core::style::Style;

/// A collection of `ratatui_core::style::Style`s consumed by the renderer.
///
/// The trait purposefully stays tiny: whenever the renderer needs a color choice we add a new
/// getter here. The default implementation maintains full backward-compatibility with the styles
/// that lived in the old `mod styles`.
pub trait StyleSheet: Clone + Send + Sync + 'static {
    /// Style for a Markdown heading.
    ///
    /// `level` is one-based (`1` for `# H1`, …).
    fn heading(&self, level: u8) -> Style;

    /// Style for inline `code` spans and fenced code blocks when syntax highlighting is disabled.
    fn code(&self) -> Style;

    /// Style for the text of an inline or reference link.
    fn link(&self) -> Style;

    /// Base style applied to blockquotes (`>` prefix and body text).
    fn blockquote(&self) -> Style;

    /// Style for heading attribute metadata appended to the heading text.
    fn heading_meta(&self) -> Style;

    /// Style for metadata blocks (front matter).
    fn metadata_block(&self) -> Style;

    /// Style for the alt-text fallback when rendering images.
    fn image_alt(&self) -> Style;

    /// Style for the table header row (bold, prominent).
    fn table_header(&self) -> Style;

    /// Style for table border characters (box-drawing glyphs).
    fn table_border(&self) -> Style;

    /// Style for a GFM alert/callout blockquote.
    ///
    /// `kind` is one of `"note"`, `"tip"`, `"important"`, `"warning"`, `"caution"`.
    fn alert(&self, kind: &str) -> Style {
        use ratatui_core::style::Color;
        match kind {
            "note" => Style::new().fg(Color::Blue),
            "tip" => Style::new().fg(Color::Green),
            "important" => Style::new().fg(Color::Magenta),
            "warning" => Style::new().fg(Color::Yellow),
            "caution" => Style::new().fg(Color::Red),
            _ => Style::default(),
        }
    }

    /// Style for raw HTML blocks and inline HTML tags.
    fn html(&self) -> Style {
        Style::new().dim()
    }

    /// Style for inline math (`$...$`).
    fn math_inline(&self) -> Style {
        Style::new().italic().magenta()
    }

    /// Style for display math (`$$...$$`).
    fn math_display(&self) -> Style {
        Style::new().magenta()
    }

    /// Style for footnote references (`[^label]`).
    fn footnote_ref(&self) -> Style {
        Style::new().dim().italic()
    }

    /// Style for footnote definitions.
    fn footnote_def(&self) -> Style {
        Style::new().dim()
    }

    /// Style for definition list terms.
    fn definition_title(&self) -> Style {
        Style::new().bold()
    }

    /// Style for definition list descriptions.
    fn definition_desc(&self) -> Style {
        Style::default()
    }

    /// Style for horizontal rules (`---`).
    fn rule(&self) -> Style {
        Style::new().dark_gray()
    }

    /// Style for code block fence markers (`` ``` ``).
    fn code_fence(&self) -> Style {
        Style::new().dark_gray()
    }

    /// Style for code block line number gutters.
    fn code_line_number(&self) -> Style {
        Style::new().dark_gray()
    }
}

/// The default style set
///
/// This style sheet will be used by default if the user does not provide their own implementation.
///
/// Styles are:
/// - H1: white on cyan, bold, underlined
/// - H2: cyan, bold
/// - H3: cyan, bold, italic
/// - H4-H6: light cyan, italic
/// - code: white on black
/// - link: blue, underlined
/// - blockquote: green
/// - metadata block: light yellow
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultStyleSheet;

impl StyleSheet for DefaultStyleSheet {
    fn heading(&self, level: u8) -> Style {
        match level {
            1 => Style::new().on_cyan().bold().underlined(),
            2 => Style::new().cyan().bold(),
            3 => Style::new().cyan().bold().italic(),
            4 => Style::new().light_cyan().italic(),
            5 => Style::new().light_cyan().italic(),
            _ => Style::new().light_cyan().italic(),
        }
    }

    fn code(&self) -> Style {
        Style::new().white().on_black()
    }

    fn link(&self) -> Style {
        Style::new().blue().underlined()
    }

    fn blockquote(&self) -> Style {
        Style::new().green()
    }

    fn heading_meta(&self) -> Style {
        // De-emphasize metadata so the heading text stays primary.
        Style::new().dim()
    }

    fn metadata_block(&self) -> Style {
        Style::new().light_yellow()
    }

    fn image_alt(&self) -> Style {
        Style::new().dim().italic()
    }

    fn table_header(&self) -> Style {
        Style::new().bold().cyan()
    }

    fn table_border(&self) -> Style {
        Style::new().dark_gray()
    }
}
