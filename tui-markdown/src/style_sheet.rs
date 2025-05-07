//! Style sheet abstraction for tui-markdown.
//!
//! The library used to hard–code all colour and attribute choices in an
//! internal `styles` module.  That made it impossible for downstream crates to
//! provide their own look-and-feel without forking the crate.  This file
//! introduces the [`StyleSheet`] trait: a minimal interface that supplies the
//! handful of [`ratatui::style::Style`] values the renderer needs.
//!
//! Users that are happy with the stock colours do not have to do anything –
//! the crate exports a [`DefaultStyleSheet`] which matches the old behaviour
//! and is used by default.  Projects that want to theme the output can
//! implement the trait for their own type and pass an instance via
//! [`crate::Options`].

use ratatui::style::{Color, Modifier, Style};

/// A collection of `ratatui::style::Style`s consumed by the renderer.
///
/// The trait purposefully stays tiny: whenever the renderer needs a colour
/// choice we add a new getter here.  The default implementation maintains full
/// backward-compatibility with the styles that lived in the old `mod styles`.
pub trait StyleSheet: Clone + Send + Sync + 'static {
    /// Style for a Markdown heading.  `level` is one-based (`1` for `# H1`, …).
    fn heading(&self, level: u8) -> Style;

    /// Style for inline `code` spans and fenced code blocks when syntax
    /// highlighting is disabled.
    fn code(&self) -> Style;

    /// Style for the text of an inline or reference link.
    fn link(&self) -> Style;

    /// Base style applied to blockquotes (`>` prefix and body text).
    fn blockquote(&self) -> Style;
}

/* ------------------------------------------------------------------------- */
/*  Built-in style sheet – identical to the previous hard-coded colours       */
/* ------------------------------------------------------------------------- */

/// The original style set that shipped with tui-markdown ≤ 0.3.
///
/// Kept as a separate type so crates can `derive(Clone)` on their own theme
/// and still fall back to this one through [`crate::Options::default`].
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultStyleSheet;

impl StyleSheet for DefaultStyleSheet {
    fn heading(&self, level: u8) -> Style {
        match level {
            1 => Style::new()
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
            2 => Style::new()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            3 => Style::new()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::ITALIC),
            4 => Style::new()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::ITALIC),
            5 => Style::new()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::ITALIC),
            _ => Style::new()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::ITALIC),
        }
    }

    fn code(&self) -> Style {
        Style::new().fg(Color::White).bg(Color::Black)
    }

    fn link(&self) -> Style {
        Style::new().fg(Color::Blue).add_modifier(Modifier::UNDERLINED)
    }

    fn blockquote(&self) -> Style {
        Style::new().fg(Color::Green)
    }
}

/// Re-export used by the crate root for convenience.
pub type BuiltinStyleSheet = DefaultStyleSheet;

