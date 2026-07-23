//! Syntax-highlighting themes for fenced code blocks.
//!
//! [`CodeTheme`] represents a theme that is ready for the renderer to use. How the theme was
//! obtained is deliberately not part of the rendering options: built-in themes and themes loaded
//! from other sources have the same type once constructed.

use std::sync::LazyLock;

use syntect::highlighting::{Theme, ThemeSet};

/// A syntax-highlighting theme that is ready to render fenced code blocks.
///
/// Construct a bundled theme with [`CodeTheme::builtin`], then pass it to
/// [`Options::code_theme`](crate::Options::code_theme). The default is
/// [`BuiltinCodeTheme::Base16OceanDark`].
///
/// This type hides the syntax-highlighting backend so applications do not need to depend on its
/// types or version.
#[derive(Clone, Debug)]
pub struct CodeTheme {
    backend: CodeThemeBackend,
}

#[derive(Clone, Debug)]
enum CodeThemeBackend {
    Builtin(BuiltinCodeTheme),
}

impl CodeTheme {
    /// Constructs a theme from one bundled with tui-markdown.
    #[must_use]
    pub fn builtin(theme: BuiltinCodeTheme) -> Self {
        Self {
            backend: CodeThemeBackend::Builtin(theme),
        }
    }
}

impl Default for CodeTheme {
    fn default() -> Self {
        Self::builtin(BuiltinCodeTheme::default())
    }
}

/// A syntax-highlighting theme bundled with tui-markdown.
///
/// Use this type to select a bundled theme, then construct a renderer-ready [`CodeTheme`] with
/// [`CodeTheme::builtin`].
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
    fn backend_name(self) -> &'static str {
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

/// Returns the backend theme represented by a public theme.
pub fn backend_theme(theme: &CodeTheme) -> &Theme {
    match &theme.backend {
        CodeThemeBackend::Builtin(theme) => builtin_theme(*theme),
    }
}

/// Returns the unresolved default theme used until options are applied.
pub fn default_code_theme() -> &'static CodeTheme {
    static DEFAULT: CodeTheme = CodeTheme {
        backend: CodeThemeBackend::Builtin(BuiltinCodeTheme::Base16OceanDark),
    };
    &DEFAULT
}

fn builtin_theme(theme: BuiltinCodeTheme) -> &'static Theme {
    THEMES
        .themes
        .get(theme.backend_name())
        .expect("every BuiltinCodeTheme variant must map to a bundled theme")
}

static THEMES: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocean_dark_is_the_default() {
        let default = CodeTheme::default();

        assert!(matches!(
            default.backend,
            CodeThemeBackend::Builtin(BuiltinCodeTheme::Base16OceanDark)
        ));
    }

    #[test]
    fn every_builtin_theme_has_a_backend_theme() {
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
            let _ = backend_theme(&theme);
        }
    }
}
