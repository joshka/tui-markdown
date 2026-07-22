//! Style sheet abstraction for tui-markdown.
//!
//! The library used to hard–code all color and attribute choices in an internal `styles` module.
//! That made it impossible for downstream crates to provide their own look-and-feel. The
//! [`StyleSheet`] trait makes it possible to customize the styles and symbols the renderer uses.
//!
//! Users that are happy with the stock colors do not have to do anything – the crate exports a
//! [`DefaultStyleSheet`] which matches the old behaviour and is used by default. Projects that want
//! to theme the output can implement the trait for their own type and pass an instance via
//! [`crate::Options`].

use ratatui_core::style::Style;

/// The kind of a GitHub Flavored Markdown alert.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AlertKind {
    /// Supplementary information.
    Note,
    /// Helpful advice.
    Tip,
    /// Information essential to success.
    Important,
    /// Urgent information that needs attention.
    Warning,
    /// A risk or negative outcome.
    Caution,
}

impl AlertKind {
    /// The canonical English label for this alert kind.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Note => "Note",
            Self::Tip => "Tip",
            Self::Important => "Important",
            Self::Warning => "Warning",
            Self::Caution => "Caution",
        }
    }
}

/// Visual styles and symbols consumed by the renderer.
///
/// The trait purposefully stays tiny: whenever the renderer needs a presentation choice we add a
/// new getter here. The default implementation maintains the renderer's standard appearance.
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
    fn definition_term(&self) -> Style {
        Style::new().bold()
    }

    /// Style for definition list descriptions.
    fn definition_description(&self) -> Style {
        Style::default()
    }
    /// Style for a GFM alert heading and body.
    ///
    /// The generated icon and label are bold in addition to this base style.
    fn alert(&self, kind: AlertKind) -> Style {
        use ratatui_core::style::Color;

        match kind {
            AlertKind::Note => Style::new().fg(Color::Blue),
            AlertKind::Tip => Style::new().fg(Color::Green),
            AlertKind::Important => Style::new().fg(Color::Magenta),
            AlertKind::Warning => Style::new().fg(Color::Yellow),
            AlertKind::Caution => Style::new().fg(Color::Red),
        }
    }

    /// Icon displayed before a GFM alert label.
    ///
    /// Return an empty string to render the label without an icon.
    fn alert_icon(&self, kind: AlertKind) -> &str {
        match kind {
            AlertKind::Note => "\u{2139}\u{FE0F}",
            AlertKind::Tip => "\u{1F4A1}",
            AlertKind::Important => "\u{2757}",
            AlertKind::Warning => "\u{26A0}\u{FE0F}",
            AlertKind::Caution => "\u{1F534}",
        }
    }

    /// Label displayed after a GFM alert icon.
    ///
    /// Return an empty string to render the icon without a label.
    fn alert_label(&self, kind: AlertKind) -> &str {
        kind.label()
    }

    /// Style for the table header row.
    fn table_header(&self) -> Style;

    /// Style for table border characters.
    fn table_border(&self) -> Style;
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
/// - raw HTML: dim
/// - inline math: magenta, italic
/// - display math: magenta
/// - footnote references: dim, italic
/// - footnote definitions: dim
/// - definition list terms: bold
/// - definition list descriptions: the surrounding style
/// - note alerts: blue
/// - tip alerts: green
/// - important alerts: magenta
/// - warning alerts: yellow
/// - caution alerts: red
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

    fn table_header(&self) -> Style {
        Style::new().bold().cyan()
    }

    fn table_border(&self) -> Style {
        Style::new().dark_gray()
    }
}
