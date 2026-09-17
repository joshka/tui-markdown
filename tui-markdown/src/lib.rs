//! Convert Markdown into Ratatui [`Text`](ratatui_core::text::Text).
//!
//! - Use [`from_str`] to render a complete string with default settings.
//! - Use [`from_str_with_options`] to choose styles, image text, code-highlighting themes, and width.
//! - Use [`StreamingMarkdown`] when text arrives in pieces. Append new text, read the current
//!   rendered output, and call [`StreamingMarkdown::finish`] when input ends.
//!
//! Batch and streaming accept the same [`Options`]. Set [`Options::width`] to wrap long lines
//! to your text area's width. The default does not wrap lines to a width.
//!
//! The returned text may borrow from the Markdown input. It contains terminal text and styles only;
//! image syntax produces a configurable text fallback and does not read or render image resources.
//!
//! # Markdown output
//!
//! Tables use Unicode box-drawing borders, terminal display widths, and the alignment declared by
//! the Markdown delimiter row. Raw HTML stays visible as literal text. Math retains its delimiters,
//! and images render as `[img]` followed by their description or destination.
//!
//! # Syntax highlighting
//!
//! The default `highlight-code` feature highlights fenced code blocks whose language is recognized.
//! It uses `Base16OceanDark` unless [`Options`] selects another [`CodeTheme`]. Themes can come from
//! the built-in set, TextMate source bundled with the application, or a TextMate file read before
//! rendering. Unrecognized code fences use [`StyleSheet::code`] instead.
#![cfg_attr(feature = "document-features", doc = "\n# Features")]
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]
//!
//! # Example
//!
//! ~~~
//! use ratatui::text::Text;
//! use tui_markdown::from_str;
//!
//! # fn draw(frame: &mut ratatui::Frame) {
//! let markdown = r#"
//! This is a simple markdown renderer for Ratatui.
//!
//! - List item 1
//! - List item 2
//!
//! ```rust
//! fn main() {
//!     println!("Hello, world!");
//! }
//! ```
//! "#;
//!
//! let text = from_str(markdown);
//! frame.render_widget(text, frame.area());
//! # }
//! ~~~

#[cfg(feature = "highlight-code")]
mod code_theme;
mod layout;
mod options;
mod renderer;
mod streaming;
mod style_sheet;

#[doc(inline)]
#[cfg(feature = "highlight-code")]
pub use crate::code_theme::{BuiltinCodeTheme, CodeTheme, CodeThemeLoadError};
pub use crate::layout::{InvalidReplacementCharacter, TableLimits};
pub use crate::options::{ImageFallback, Options};
pub use crate::renderer::{from_str, from_str_with_options};
pub use crate::streaming::{
    ChangeReason, PreparedRows, ResourceUsage, StreamingMarkdown, TableFallbacks, Update,
    WorkCounters,
};
pub use crate::style_sheet::{AlertKind, DefaultStyleSheet, StyleSheet};
