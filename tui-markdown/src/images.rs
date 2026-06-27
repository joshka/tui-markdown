//! Image rendering support for tui-markdown.
//!
//! By default, images are rendered as alt-text fallback:
//!
//! ```text
//! 🖼 alt text
//! ```
//!
//! When the `terminal-images` feature is enabled, the renderer can use terminal
//! image protocols (iTerm2/Kitty) to display inline images via the `ratatui-image`
//! crate. This is not yet implemented.

/// Image indicator prepended to alt text or URL in fallback mode.
pub const IMAGE_INDICATOR: &str = "[img]";

// TODO: When the `terminal-images` feature is implemented, add:
// - Terminal capability detection (iTerm2, Kitty, Sixel)
// - Image loading and decoding via the `image` crate
// - Inline rendering via `ratatui-image`
// - Graceful fallback to alt-text mode when the terminal does not support images
