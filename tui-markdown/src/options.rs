//! Rendering configuration for tui-markdown.
//!
//! [`Options`] is `#[non_exhaustive]`, so new fields can be added without breaking downstream
//! code.  Use [`Options::new`] or [`Options::default`] to construct, and builder methods such as
//! [`Options::code_theme`] to customise individual settings.

#[cfg(feature = "highlight-code")]
use std::path::Path;

use crate::{DefaultStyleSheet, StyleSheet};

/// Collection of optional parameters that influence markdown rendering.
///
/// The generic `S` allows users to provide their own theme type that implements the
/// [`StyleSheet`] trait. The default implementation is [`DefaultStyleSheet`], which provides a
/// set of sensible defaults for styling markdown elements.
///
/// # Example
///
/// ```
/// use tui_markdown::{DefaultStyleSheet, Options};
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
///     fn heading(&self, level: u8) -> Style {
///         Style::new().bold()
///     }
///
///     fn code(&self) -> Style {
///         Style::new().white().on_dark_gray()
///     }
///
///     fn link(&self) -> Style {
///         Style::new().blue().underlined()
///     }
///
///     fn blockquote(&self) -> Style {
///         Style::new().yellow()
///     }
///
///     fn heading_meta(&self) -> Style {
///         Style::new().dim()
///     }
///
///     fn metadata_block(&self) -> Style {
///         Style::new().light_yellow()
///     }
///
///     fn image_alt(&self) -> Style {
///         Style::new().dim().italic()
///     }
///
///     fn table_header(&self) -> Style {
///         Style::new().bold().cyan()
///     }
///
///     fn table_border(&self) -> Style {
///         Style::new().dark_gray()
///     }
/// }
///
/// let options = Options::new(MyStyleSheet).code_theme("Solarized (dark)");
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Options<S: StyleSheet = DefaultStyleSheet> {
    /// The [`StyleSheet`] implementation that will be consulted every time the renderer needs a
    /// color choice.
    pub(crate) styles: S,

    /// Name of the syntect theme used for syntax-highlighted code blocks.
    ///
    /// Looked up in `syntect::highlighting::ThemeSet::load_defaults()`.
    /// When the `highlight-code` feature is disabled this field has no effect.
    ///
    /// Defaults to [`Self::DEFAULT_CODE_THEME`].
    pub(crate) code_theme: String,

    /// Optional custom theme loaded from a `.tmTheme` file, taking precedence over
    /// [`Self::code_theme`] when set.
    #[cfg(feature = "highlight-code")]
    pub(crate) code_theme_override: Option<syntect::highlighting::Theme>,
}

impl<S: StyleSheet> Options<S> {
    /// The default syntax highlighting theme name.
    pub const DEFAULT_CODE_THEME: &str = "base16-ocean.dark";

    /// Creates a new `Options` instance with the provided style sheet.
    pub fn new(styles: S) -> Self {
        Self {
            styles,
            code_theme: Self::DEFAULT_CODE_THEME.to_owned(),
            #[cfg(feature = "highlight-code")]
            code_theme_override: None,
        }
    }

    /// Set the syntax highlighting theme by name.
    ///
    /// The name must match a key in `syntect::highlighting::ThemeSet::load_defaults()`.
    /// Built-in theme names include `"base16-ocean.dark"`, `"base16-eighties.dark"`,
    /// `"Solarized (dark)"`, `"Solarized (light)"`, `"InspiredGitHub"`, and others.
    ///
    /// Use [`crate::available_themes`] to list all valid names at runtime.
    ///
    /// Has no effect when the `highlight-code` feature is disabled.
    pub fn code_theme(mut self, theme_name: impl Into<String>) -> Self {
        self.code_theme = theme_name.into();
        #[cfg(feature = "highlight-code")]
        {
            self.code_theme_override = None;
        }
        self
    }

    /// Load a custom syntax highlighting theme from a `.tmTheme` file.
    ///
    /// This takes precedence over the built-in theme set by [`Self::code_theme`].
    /// Hundreds of `.tmTheme` files are available from editors like Sublime Text,
    /// TextMate, and VS Code.
    ///
    /// # Errors
    ///
    /// Returns [`syntect::LoadingError`] if the file cannot be read or parsed.
    #[cfg(feature = "highlight-code")]
    pub fn code_theme_file(mut self, path: impl AsRef<Path>) -> Result<Self, syntect::LoadingError> {
        let theme = syntect::highlighting::ThemeSet::get_theme(path)?;
        self.code_theme_override = Some(theme);
        Ok(self)
    }

    /// Use a custom [`syntect::highlighting::Theme`] directly.
    ///
    /// This takes precedence over the built-in theme set by [`Self::code_theme`].
    #[cfg(feature = "highlight-code")]
    pub fn code_theme_custom(mut self, theme: syntect::highlighting::Theme) -> Self {
        self.code_theme_override = Some(theme);
        self
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

            fn code(&self) -> Style {
                Style::new().white().on_black()
            }

            fn link(&self) -> Style {
                Style::new().blue().underlined()
            }

            fn blockquote(&self) -> Style {
                Style::new().yellow()
            }

            fn heading_meta(&self) -> Style {
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

        let options = Options::new(CustomStyleSheet);

        assert_eq!(options.styles.heading(1), Style::new().red().bold());
        assert_eq!(options.styles.heading(2), Style::new().green());
        assert_eq!(options.styles.code(), Style::new().white().on_black());
        assert_eq!(options.styles.link(), Style::new().blue().underlined());
        assert_eq!(options.styles.blockquote(), Style::new().yellow());
        assert_eq!(options.styles.heading_meta(), Style::new().dim());
        assert_eq!(options.styles.metadata_block(), Style::new().light_yellow());
        assert_eq!(options.styles.image_alt(), Style::new().dim().italic());
    }

    #[test]
    fn default_code_theme() {
        let options: Options = Options::default();
        assert_eq!(options.code_theme, Options::<DefaultStyleSheet>::DEFAULT_CODE_THEME);
    }

    #[test]
    fn custom_code_theme() {
        let options = Options::default().code_theme("Solarized (dark)");
        assert_eq!(options.code_theme, "Solarized (dark)");
    }
}
