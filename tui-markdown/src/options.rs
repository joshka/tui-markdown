//! Rendering configuration for tui-markdown.
//!
//! Options control the renderer's style sheet and image fallback content. This struct is
//! `#[non_exhaustive]`, which allows us to add more options in the future without breaking
//! existing code.

use crate::{DefaultStyleSheet, StyleSheet};

/// Text used in place of images when rendering Markdown in a terminal.
#[non_exhaustive]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ImageFallback {
    /// Show the image's alt text, or its URL when the alt text is empty.
    #[default]
    AltText,
    /// Always show the image's URL.
    Url,
    /// Show both the image's alt text and URL, or only its URL when the alt text is empty.
    AltTextAndUrl,
}

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
    /// The content to render in place of images.
    pub(crate) image_fallback: ImageFallback,
}

impl<S: StyleSheet> Options<S> {
    /// Creates a new `Options` instance with the provided style sheet.
    pub fn new(styles: S) -> Self {
        Self {
            styles,
            image_fallback: ImageFallback::default(),
        }
    }

    /// Selects the content to render in place of images.
    #[must_use]
    pub fn image_fallback(mut self, image_fallback: ImageFallback) -> Self {
        self.image_fallback = image_fallback;
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
        }

        let options = Options {
            styles: CustomStyleSheet,
            image_fallback: ImageFallback::default(),
        };

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
    fn image_fallback_defaults_to_alt_text() {
        let options = Options::default();

        assert_eq!(options.image_fallback, ImageFallback::AltText);
    }

    #[test]
    fn image_fallback_setter_updates_mode() {
        let options = Options::default().image_fallback(ImageFallback::AltTextAndUrl);

        assert_eq!(options.image_fallback, ImageFallback::AltTextAndUrl);
    }
}
