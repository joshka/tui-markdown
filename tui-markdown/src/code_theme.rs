//! Built-in syntax-highlighting theme discovery and selection.
//!
//! Resolve the configured name before parsing events so an unknown name produces one warning per
//! rendered document rather than one warning for every fenced code block.

use std::sync::LazyLock;

use syntect::highlighting::{Theme, ThemeSet};
use tracing::warn;

use crate::DEFAULT_CODE_THEME;

static THEMES: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// Returns the names of all built-in syntax-highlighting themes.
///
/// Pass one of these names to [`crate::Options::code_theme`]. Code blocks use
/// [`DEFAULT_CODE_THEME`] when no theme is selected.
///
/// This function is available with the `highlight-code` feature, which is enabled by default.
///
/// # Example
///
/// ```
/// use tui_markdown::{available_themes, Options};
///
/// assert!(available_themes().contains(&"InspiredGitHub"));
/// let options = Options::default().code_theme("InspiredGitHub");
/// ```
pub fn available_themes() -> Vec<&'static str> {
    THEMES.themes.keys().map(String::as_str).collect()
}

/// Returns the default built-in theme.
pub fn default_theme() -> &'static Theme {
    &THEMES.themes[DEFAULT_CODE_THEME]
}

/// Resolves a requested theme name, falling back to the default when the name is unknown.
pub fn resolve(name: &str) -> &'static Theme {
    THEMES.themes.get(name).unwrap_or_else(|| {
        warn!("Theme {name:?} not found, falling back to {DEFAULT_CODE_THEME:?}");
        default_theme()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_themes_include_the_default_and_documented_example() {
        let themes = available_themes();

        assert!(themes.contains(&DEFAULT_CODE_THEME));
        assert!(themes.contains(&"InspiredGitHub"));
    }
}
