//! Built-in syntax-highlighting themes for fenced code blocks.
//!
//! Construct a [`CodeTheme`] with [`CodeTheme::builtin`], then select it with
//! [`Options::code_theme`](crate::Options::code_theme). The renderer consults the theme when a
//! fenced code block names a recognized language.
//!
//! A configured [`CodeTheme`] owns its theme data. [`Options`](crate::Options) stores no theme by
//! default; the renderer instead borrows the shared [`BuiltinCodeTheme::Base16OceanDark`] theme
//! when it first encounters a recognized fenced language.

use std::sync::LazyLock;

use syntect::highlighting::{Theme, ThemeSet};

/// An owned syntax-highlighting theme for fenced code blocks.
///
/// Construct a bundled theme with [`CodeTheme::builtin`], then pass it to
/// [`Options::code_theme`](crate::Options::code_theme).
///
/// The renderer consults the theme only for fenced code blocks whose language is recognized.
///
/// This type hides the syntax-highlighting backend so applications do not need to depend on its
/// types or version.
///
/// [`CodeTheme::default`] constructs an owned [`BuiltinCodeTheme::Base16OceanDark`] theme. Default
/// [`Options`](crate::Options), however, leave the theme unset so the renderer can borrow its shared
/// copy of that theme.
#[derive(Clone, Debug)]
pub struct CodeTheme {
    theme: Theme,
}

impl CodeTheme {
    /// Constructs an owned syntax-highlighting theme from a bundled theme.
    #[must_use]
    pub fn builtin(theme: BuiltinCodeTheme) -> Self {
        let theme = builtin_theme(theme).clone();
        Self { theme }
    }
}

impl Default for CodeTheme {
    fn default() -> Self {
        Self::builtin(BuiltinCodeTheme::default())
    }
}

/// A syntax-highlighting theme bundled with tui-markdown.
///
/// Pass a variant to [`CodeTheme::builtin`] and select the resulting theme with
/// [`Options::code_theme`](crate::Options::code_theme).
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

/// Returns the configured theme or the shared default.
///
/// The renderer calls this only after recognizing a fenced language. Keeping the choice here means
/// ordinary Markdown and unrecognized code fences do not initialize the bundled theme set, while
/// default options can borrow the shared theme instead of cloning it.
pub fn theme_or_default(theme: Option<&CodeTheme>) -> &Theme {
    match theme {
        Some(theme) => &theme.theme,
        None => default_theme(),
    }
}

fn builtin_theme(theme: BuiltinCodeTheme) -> &'static Theme {
    THEMES
        .themes
        .get(theme.syntect_name())
        .expect("every BuiltinCodeTheme variant must map to a bundled theme")
}

fn default_theme() -> &'static Theme {
    builtin_theme(BuiltinCodeTheme::default())
}

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

        for theme in themes {
            let theme = CodeTheme::builtin(theme);
            let _ = theme_or_default(Some(&theme));
        }
    }

    #[test]
    fn configured_theme_is_borrowed_directly() {
        let theme = CodeTheme::builtin(BuiltinCodeTheme::SolarizedDark);

        assert!(std::ptr::eq(theme_or_default(Some(&theme)), &theme.theme));
    }

    #[test]
    fn absent_theme_uses_shared_default() {
        assert!(std::ptr::eq(theme_or_default(None), default_theme()));
    }
}
