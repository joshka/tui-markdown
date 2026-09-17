//! Choose how batch and streaming Markdown output is displayed.
//!
//! [`Options`] controls styles, the text shown for images, code-highlighting themes, and optional
//! width wrapping. Default options do not wrap long lines to a width.
//! The type is non-exhaustive so new choices can be added without breaking existing code.

use crate::layout::LayoutOptions;
#[cfg(feature = "highlight-code")]
use crate::CodeTheme;
use crate::{DefaultStyleSheet, InvalidReplacementCharacter, StyleSheet, TableLimits};

/// Text used to represent Markdown images in rendered terminal output.
///
/// This option does not load or render image resources. It controls whether the text fallback
/// contains the image description, destination, or both. [`AltText`](Self::AltText) is the
/// default.
///
/// # Example
///
/// ```
/// use tui_markdown::{from_str_with_options, ImageFallback, Options};
///
/// let options = Options::default().image_fallback(ImageFallback::AltTextAndUrl);
/// let text = from_str_with_options("![Architecture diagram](diagram.png)", &options);
///
/// assert_eq!(
///     text.to_string(),
///     "[img] Architecture diagram (diagram.png)"
/// );
/// ```
#[non_exhaustive]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ImageFallback {
    /// Show `[img]` followed by the description, or the destination when the description is empty.
    #[default]
    AltText,
    /// Show `[img]` followed by the destination, ignoring the description.
    Url,
    /// Show `[img] {description} ({destination})`, omitting either value when it is empty.
    AltTextAndUrl,
}

/// Choose how Markdown looks when rendering a complete string or receiving text in pieces.
///
/// Pass the same options to [`crate::from_str_with_options`] or [`crate::StreamingMarkdown::new`].
/// Use [`Self::width`] to wrap text to your UI's available width. By default, long lines are not
/// wrapped to a width; existing batch callers keep their original behavior.
///
/// `S` is the style sheet consulted while Markdown events are rendered. [`Options::default`] uses
/// [`DefaultStyleSheet`]. Use [`Options::new`] to supply another [`StyleSheet`].
/// [`StyleSheet::heading_marker`] and [`StyleSheet::code_block_fence`] customize or hide the
/// corresponding presentation symbols.
///
/// # Example
///
/// ```
/// use tui_markdown::Options;
///
/// let options = Options::default();
///
/// // or with a custom style sheet
///
/// use ratatui_core::style::{Style, Stylize};
/// use tui_markdown::StyleSheet;
///
/// #[derive(Debug, Clone)]
/// struct MyStyleSheet;
///
/// impl StyleSheet for MyStyleSheet {
///     fn heading(&self, _level: u8) -> Style {
///         Style::new().bold()
///     }
/// }
///
/// let options = Options::new(MyStyleSheet);
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Options<S: StyleSheet = DefaultStyleSheet> {
    pub(crate) layout: LayoutOptions,
    /// The [`StyleSheet`] implementation that will be consulted every time the renderer needs a
    /// style or symbol choice.
    pub(crate) styles: S,
    /// The content to render in place of images.
    pub(crate) image_fallback: ImageFallback,
    /// Explicit syntax-highlighting theme for fenced code blocks.
    ///
    /// When absent, the renderer uses the shared built-in default.
    #[cfg(feature = "highlight-code")]
    code_theme: Option<CodeTheme>,
}

impl<S: StyleSheet> Options<S> {
    /// Creates rendering options that use `styles`.
    ///
    /// Other settings keep their defaults, including no width wrapping.
    pub fn new(styles: S) -> Self {
        Self {
            layout: LayoutOptions::default(),
            styles,
            image_fallback: ImageFallback::default(),
            #[cfg(feature = "highlight-code")]
            code_theme: None,
        }
    }

    /// Sets the space available for Markdown text, measured in terminal cells (columns).
    ///
    /// - `None` is the default. Markdown is still rendered, but long lines are not wrapped to a
    ///   width. Table buffer limits and the overwide-character replacement do not affect output.
    /// - `Some(0)` produces no display rows. It does not mean unlimited width.
    /// - `Some(80)`, for example, allows 80 cells per row. Most ASCII characters use one cell;
    ///   many CJK characters and emoji use two.
    ///
    /// Supply the actual text area width, excluding your UI's reply markers and borders.
    /// The package does not read the terminal size. To resize an existing streaming document,
    /// use [`crate::StreamingMarkdown::set_width`] rather than replacing all options.
    ///
    /// Tables keep bordered grids when possible. Wide cells wrap, with horizontal separators
    /// between logical rows. Columns keep enough space for complete graphemes, plus one space
    /// of padding on each side. Short columns keep their natural widths when possible.
    /// If minimum grid geometry or [`Self::table_limits`] prevents a grid, cells are listed
    /// vertically with numbers instead. Width `None` keeps the original table layout.
    ///
    /// # Example
    ///
    /// ```
    /// use tui_markdown::{from_str_with_options, Options};
    ///
    /// let options = Options::default().width(Some(4));
    /// assert_eq!(from_str_with_options("abcdefghij", &options).to_string(), "abcd\nefgh\nij");
    /// ```
    #[must_use]
    pub fn width(mut self, width: Option<u16>) -> Self {
        self.layout = self.layout.with_width(width);
        self
    }

    /// Sets how much data the renderer buffers while building a table grid.
    ///
    /// These limits apply only when [`Self::width`] is `Some`. If a grid exceeds either limit,
    /// the renderer lists the table's cells vertically with numbers and keeps their content.
    /// Set either limit to zero to request this presentation for every table.
    ///
    /// See [`TableLimits`] for defaults. These limits do not cap source storage, rendered output,
    /// or total process memory. With width `None`, the values are stored but do not change output.
    #[must_use]
    pub fn table_limits(mut self, limits: TableLimits) -> Self {
        self.layout = self.layout.table_limits(limits);
        self
    }

    /// Chooses the character shown when one grapheme is wider than the entire available row.
    ///
    /// A grapheme is one displayed character, such as a letter with an accent or a joined emoji.
    /// If it fits the row but not the remaining space, it wraps unchanged to the next row.
    /// If it cannot fit anywhere in the row, the renderer uses this replacement, defaulting to `-`.
    ///
    /// The replacement keeps the original style. Source text is never changed, so widening or
    /// disabling wrapping restores the original character. Width `None` does not use replacements.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidReplacementCharacter`] unless `replacement` is printable ASCII
    /// (`U+0020` through `U+007E`).
    ///
    /// # Example
    ///
    /// ```
    /// use tui_markdown::{from_str_with_options, Options};
    ///
    /// let options = Options::default().width(Some(1)).with_wide_grapheme_replacement('*')?;
    /// assert_eq!(from_str_with_options("\u{754c}", &options).to_string(), "*");
    /// # Ok::<(), tui_markdown::InvalidReplacementCharacter>(())
    /// ```
    pub fn with_wide_grapheme_replacement(
        mut self,
        replacement: char,
    ) -> Result<Self, InvalidReplacementCharacter> {
        self.layout = self.layout.with_wide_grapheme_replacement(replacement)?;
        Ok(self)
    }

    /// Selects the text used to represent Markdown images.
    ///
    /// See [`ImageFallback`] for the exact output of each mode.
    #[must_use]
    pub fn image_fallback(mut self, image_fallback: ImageFallback) -> Self {
        self.image_fallback = image_fallback;
        self
    }

    /// Selects the syntax-highlighting theme for fenced code blocks.
    ///
    /// By default, no explicit theme is stored and the renderer borrows its shared
    /// [`Base16OceanDark`](crate::BuiltinCodeTheme::Base16OceanDark) theme.
    /// Pass a [`BuiltinCodeTheme`](crate::BuiltinCodeTheme) directly, or pass an owned
    /// [`CodeTheme`]. Construct custom themes from TextMate source with
    /// [`CodeTheme::from_textmate`](crate::CodeTheme::from_textmate), or load a TextMate file with
    /// [`CodeTheme::from_file`](crate::CodeTheme::from_file). The selected theme applies when a
    /// fenced code block names a recognized language.
    ///
    /// # Example
    ///
    /// ```
    /// use tui_markdown::{BuiltinCodeTheme, Options};
    ///
    /// let options = Options::default().code_theme(BuiltinCodeTheme::SolarizedDark);
    /// ```
    #[cfg(feature = "highlight-code")]
    #[must_use]
    pub fn code_theme(mut self, code_theme: impl Into<CodeTheme>) -> Self {
        self.code_theme = Some(code_theme.into());
        self
    }

    /// Returns the explicitly configured syntax-highlighting theme.
    ///
    /// Returns `None` when the renderer will use the shared
    /// [`Base16OceanDark`](crate::BuiltinCodeTheme::Base16OceanDark) default.
    #[cfg(feature = "highlight-code")]
    #[must_use]
    pub fn selected_code_theme(&self) -> Option<&CodeTheme> {
        self.code_theme.as_ref()
    }
}

impl Default for Options<DefaultStyleSheet> {
    fn default() -> Self {
        Self::new(DefaultStyleSheet)
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::style::Style;

    use super::*;

    #[test]
    fn default() {
        let options: Options = Default::default();
        assert_eq!(
            options.styles.heading(1),
            Style::new().on_cyan().bold().underlined()
        );
    }

    #[test]
    fn custom_style_sheet() {
        #[derive(Debug, Clone)]
        struct CustomStyleSheet;

        impl StyleSheet for CustomStyleSheet {
            fn heading(&self, level: u8) -> Style {
                match level {
                    1 => Style::new().red().bold(),
                    _ => Style::new().green(),
                }
            }
        }

        let options = Options::new(CustomStyleSheet);

        assert_eq!(options.styles.heading(1), Style::new().red().bold());
        assert_eq!(options.styles.heading(2), Style::new().green());
        assert_eq!(options.styles.code(), Style::new().white().on_black());
        assert_eq!(options.styles.link(), Style::new().blue().underlined());
        assert_eq!(options.styles.blockquote(), Style::new().green());
        assert_eq!(options.styles.heading_meta(), Style::new().dim());
        assert_eq!(options.styles.metadata_block(), Style::new().light_yellow());
        assert_eq!(options.styles.image_alt(), Style::new().dim().italic());
    }

    #[test]
    fn image_fallback_defaults_to_alt_text() {
        let options = Options::default();

        assert_eq!(options.image_fallback, ImageFallback::AltText);
    }

    #[test]
    fn image_fallback_setter_updates_mode() {
        let options = Options::default().image_fallback(ImageFallback::AltTextAndUrl);

        assert_eq!(options.image_fallback, ImageFallback::AltTextAndUrl);
    }

    #[test]
    #[cfg(feature = "highlight-code")]
    fn default_has_no_explicit_code_theme() {
        let options: Options = Options::default();

        assert!(options.selected_code_theme().is_none());
    }

    #[test]
    #[cfg(feature = "highlight-code")]
    fn code_theme_selects_theme() {
        let options = Options::default().code_theme(crate::BuiltinCodeTheme::SolarizedDark);

        assert!(options.selected_code_theme().is_some());
    }
}
