//! Built-in syntax-highlighting themes for fenced code blocks.
//!
//! Pass a [`BuiltinCodeTheme`] to [`Options::code_theme`](crate::Options::code_theme), or convert it
//! into an owned [`CodeTheme`]. The renderer consults the theme when a fenced code block names a
//! recognized language.
//!
//! A configured [`CodeTheme`] owns its theme data. [`Options`](crate::Options) stores no theme by
//! default; the renderer instead borrows a shared [`CodeTheme`] for
//! [`BuiltinCodeTheme::Base16OceanDark`] when it first encounters a recognized fenced language.

use std::sync::LazyLock;

use syntect::highlighting::{Theme, ThemeSet};

/// An owned syntax-highlighting theme for fenced code blocks.
///
/// Convert a [`BuiltinCodeTheme`] into this type, or pass the built-in theme directly to
/// [`Options::code_theme`](crate::Options::code_theme).
///
/// The renderer consults the theme only for fenced code blocks whose language is recognized.
///
/// This type hides the syntax-highlighting implementation so applications do not need to depend on
/// its types or version.
#[derive(Clone, Debug)]
pub struct CodeTheme {
    theme: Theme,
}

/// A syntax-highlighting theme bundled with tui-markdown.
///
/// Pass a variant directly to [`Options::code_theme`](crate::Options::code_theme), or convert it
/// into an owned [`CodeTheme`].
#[non_exhaustive]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinCodeTheme {
    /// The dark Base16 Eighties theme.
    Base16EightiesDark,
    /// The dark Base16 Mocha theme.
    Base16MochaDark,
    /// The default dark Base16 Ocean theme.
    #[default]
    Base16OceanDark,
    /// The light Base16 Ocean theme.
    Base16OceanLight,
    /// The light Inspired GitHub theme.
    InspiredGitHub,
    /// The dark Solarized theme.
    SolarizedDark,
    /// The light Solarized theme.
    SolarizedLight,
}

impl BuiltinCodeTheme {
    fn syntect_name(self) -> &'static str {
        match self {
            Self::Base16EightiesDark => "base16-eighties.dark",
            Self::Base16MochaDark => "base16-mocha.dark",
            Self::Base16OceanDark => "base16-ocean.dark",
            Self::Base16OceanLight => "base16-ocean.light",
            Self::InspiredGitHub => "InspiredGitHub",
            Self::SolarizedDark => "Solarized (dark)",
            Self::SolarizedLight => "Solarized (light)",
        }
    }
}

impl From<BuiltinCodeTheme> for CodeTheme {
    fn from(theme: BuiltinCodeTheme) -> Self {
        let theme = builtin_theme(theme).clone();
        Self { theme }
    }
}

/// Returns the syntax-highlighting data for a code theme.
pub fn theme(code_theme: &CodeTheme) -> &Theme {
    &code_theme.theme
}

/// Returns the lazily initialized default code theme.
///
/// The renderer calls this only after recognizing a fenced language, so ordinary Markdown and
/// unrecognized code fences do not initialize the bundled theme set.
pub fn default() -> &'static CodeTheme {
    &DEFAULT_THEME
}

fn builtin_theme(code_theme: BuiltinCodeTheme) -> &'static Theme {
    THEMES
        .themes
        .get(code_theme.syntect_name())
        .expect("every BuiltinCodeTheme variant must map to a bundled theme")
}

static DEFAULT_THEME: LazyLock<CodeTheme> = LazyLock::new(|| BuiltinCodeTheme::default().into());
static THEMES: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_theme_can_be_selected() {
        let themes = [
            BuiltinCodeTheme::Base16EightiesDark,
            BuiltinCodeTheme::Base16MochaDark,
            BuiltinCodeTheme::Base16OceanDark,
            BuiltinCodeTheme::Base16OceanLight,
            BuiltinCodeTheme::InspiredGitHub,
            BuiltinCodeTheme::SolarizedDark,
            BuiltinCodeTheme::SolarizedLight,
        ];

        for built_in in themes {
            let code_theme = CodeTheme::from(built_in);
            let _ = theme(&code_theme);
        }
    }

    #[test]
    fn configured_theme_is_borrowed_directly() {
        let code_theme = CodeTheme::from(BuiltinCodeTheme::SolarizedDark);

        assert!(std::ptr::eq(theme(&code_theme), &code_theme.theme));
    }

    #[test]
    fn default_theme_is_shared() {
        assert!(std::ptr::eq(default(), default()));
    }
}
