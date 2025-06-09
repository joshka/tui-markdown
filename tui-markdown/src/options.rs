//! Rendering configuration for tui-markdown.
//!
//! For now the only knob is the theme [`StyleSheet`].  The struct purposefully remains
//! `#[non_exhaustive]` *in spirit* (not in syntax to avoid MSRV bumps): we recommend construction
//! through the builder pattern or the field shorthand with `..Default::default()` so that adding
//! new options becomes a non-breaking change.

use crate::style_sheet::{BuiltinStyleSheet, StyleSheet};

/// Collection of optional parameters that influence markdown rendering.
///
/// The generic `S` allows users to carry their own theme type without forcing dynamic dispatch.
/// Libraries that do not want the extra generic in their call-chain can always box it (`Box<dyn
/// StyleSheet>`).
#[derive(Clone)]
pub struct Options<S: StyleSheet = BuiltinStyleSheet> {
    /// The [`StyleSheet`] implementation that will be consulted every time the renderer needs a
    /// color choice.
    pub styles: S,

    /// Wrap text at this many columns.
    ///
    /// The default value is `None` which indicates that there is no wrapping.
    pub wrap_width: Option<u16>,

    /// Whether to show line numbers inside fenced code blocks.
    ///
    /// *Not yet implemented* – but the flag is here so that the field layout stays stable once the
    /// feature lands.
    pub show_line_numbers: bool,
}

impl Default for Options<BuiltinStyleSheet> {
    fn default() -> Self {
        Self {
            styles: crate::style_sheet::DefaultStyleSheet,
            wrap_width: None,
            show_line_numbers: false,
        }
    }
}
