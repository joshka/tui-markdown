//! Rendering configuration for tui-markdown.
//!
//! For now the only knob is the theme [`StyleSheet`]. This struct is `#[non_exhaustive]`, which
//! allows us to add more options in the future without breaking existing code.

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
/// use tui_markdown::{Options, DefaultStyleSheet};
/// let options = Options::default();
///
/// // or with a custom style sheet
///
/// use ratatui::style::{Style, Stylize};
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
/// }
///
/// let options = Options::new(MyStyleSheet);
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Options<S: StyleSheet = DefaultStyleSheet> {
    /// The [`StyleSheet`] implementation that will be consulted every time the renderer needs a
    /// color choice.
    pub(crate) styles: S,
}

impl<S: StyleSheet> Options<S> {
    /// Creates a new `Options` instance with the provided style sheet.
    pub fn new(styles: S) -> Self {
        Self { styles }
    }
}

impl Default for Options<DefaultStyleSheet> {
    fn default() -> Self {
        Self::new(DefaultStyleSheet)
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::{Style, Stylize};

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
        }

        let options = Options {
            styles: CustomStyleSheet,
        };

        assert_eq!(options.styles.heading(1), Style::new().red().bold());
        assert_eq!(options.styles.heading(2), Style::new().green());
        assert_eq!(options.styles.code(), Style::new().white().on_black());
        assert_eq!(options.styles.link(), Style::new().blue().underlined());
        assert_eq!(options.styles.blockquote(), Style::new().yellow());
    }
}
