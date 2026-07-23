//! A simple markdown renderer widget for Ratatui.
//!
//! This module provides a simple markdown renderer widget for Ratatui. It uses the `pulldown-cmark`
//! crate to parse markdown and convert it to a `Text` widget. The `Text` widget can then be
//! rendered to the terminal using the 'Ratatui' library.
//!
//! GitHub-flavored Markdown tables render with Unicode box-drawing borders, terminal-width-aware
//! columns, and the alignment declared by the Markdown delimiter row. Use [`StyleSheet`] to
//! customize header cells, body cells, and borders.
//!
//! Images render as `[img]` followed by their description, or by their destination when the
//! description is empty. This is a text fallback; the crate does not load or render image
//! resources.
//!
//! The default `highlight-code` feature highlights fenced code blocks whose language is recognized,
//! using `Base16OceanDark` unless [`Options`] selects another `CodeTheme`.
//! Themes can be bundled with tui-markdown, parsed from TextMate source, or loaded from a TextMate
//! file. Fences without a recognized language remain unhighlighted.
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
mod options;
mod renderer;
mod style_sheet;

#[doc(inline)]
#[cfg(feature = "highlight-code")]
pub use crate::code_theme::{BuiltinCodeTheme, CodeTheme, CodeThemeLoadError};
pub use crate::options::{ImageFallback, Options};
pub use crate::renderer::{from_str, from_str_with_options};
pub use crate::style_sheet::{AlertKind, DefaultStyleSheet, StyleSheet};
