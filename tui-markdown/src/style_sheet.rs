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
}
