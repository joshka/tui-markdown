//! A simple markdown renderer widget for Ratatui.
//!
//! This module provides a simple markdown renderer widget for Ratatui. It uses the `pulldown-cmark`
//! crate to parse markdown and convert it to a `Text` widget. The `Text` widget can then be
//! rendered to the terminal using the 'Ratatui' library.
#![cfg_attr(feature = "document-features", doc = "\n# Features")]
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]
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
use std::sync::LazyLock;
use std::vec;

#[cfg(feature = "highlight-code")]
use ansi_to_tui::IntoText;
use itertools::{Itertools, Position};
use pulldown_cmark::{
    BlockQuoteKind, CodeBlockKind, CowStr, Event, HeadingLevel, Options as ParseOptions, Parser,
    Tag, TagEnd,
};
use ratatui_core::style::{Style, Stylize};
use ratatui_core::text::{Line, Span, Text};
#[cfg(feature = "highlight-code")]
use syntect::{
    easy::HighlightLines,
    highlighting::ThemeSet,
    parsing::SyntaxSet,
    util::{as_24_bit_terminal_escaped, LinesWithEndings},
};
use tracing::{debug, instrument, warn};

pub use crate::content::{MarkdownBlock, MarkdownContent};
pub use crate::options::Options;
pub use crate::style_sheet::{DefaultStyleSheet, StyleSheet};

// Re-export syntect types needed by the public API so consumers don't need a direct
// syntect dependency.
#[cfg(feature = "highlight-code")]
pub use syntect::highlighting::Theme as SyntectTheme;
#[cfg(feature = "highlight-code")]
pub use syntect::LoadingError;

mod content;
mod images;
mod options;
mod style_sheet;
mod tables;

/// Render Markdown `input` into a [`ratatui::text::Text`] using the default [`Options`].
///
/// This is a convenience function that uses the default options, which are defined in
/// [`Options::default`]. It is suitable for most use cases where you want to render Markdown
pub fn from_str(input: &str) -> Text<'_> {
    from_str_with_options(input, &Options::default())
}

/// Render Markdown `input` into a [`ratatui::text::Text`] using the supplied [`Options`].
///
/// This allows you to customize the styles and other rendering options.
///
/// # Example
///
/// ```
/// use tui_markdown::{from_str_with_options, DefaultStyleSheet, Options};
///
/// let input = "This is a **bold** text.";
/// let options = Options::default();
/// let text = from_str_with_options(input, &options);
/// ```
pub fn from_str_with_options<'a, S>(input: &'a str, options: &Options<S>) -> Text<'a>
where
    S: StyleSheet,
{
    let mut parse_opts = ParseOptions::empty();
    parse_opts.insert(ParseOptions::ENABLE_STRIKETHROUGH);
    parse_opts.insert(ParseOptions::ENABLE_TASKLISTS);
    parse_opts.insert(ParseOptions::ENABLE_HEADING_ATTRIBUTES);
    parse_opts.insert(ParseOptions::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    parse_opts.insert(ParseOptions::ENABLE_SUPERSCRIPT);
    parse_opts.insert(ParseOptions::ENABLE_SUBSCRIPT);
    parse_opts.insert(ParseOptions::ENABLE_TABLES);
    parse_opts.insert(ParseOptions::ENABLE_GFM);
    parse_opts.insert(ParseOptions::ENABLE_FOOTNOTES);
    parse_opts.insert(ParseOptions::ENABLE_MATH);
    parse_opts.insert(ParseOptions::ENABLE_DEFINITION_LIST);
    let parser = Parser::new_ext(input, parse_opts);

    let mut writer = TextWriter::new(
        parser,
        options.styles.clone(),
        options.code_theme.clone(),
        #[cfg(feature = "highlight-code")]
        options.code_theme_override.clone(),
    );
    writer.run();
    writer.text
}

/// Parse Markdown `input` into a [`MarkdownContent`] using the default [`Options`].
///
/// Unlike [`from_str`], images are represented as separate [`MarkdownBlock::Image`] blocks
/// rather than being flattened to alt text. This allows consumers to render images using
/// terminal image protocols or custom rendering.
pub fn parse(input: &str) -> MarkdownContent<'_> {
    parse_with_options(input, &Options::default())
}

/// Parse Markdown `input` into a [`MarkdownContent`] using the supplied [`Options`].
///
/// See [`parse`] for details on the difference from [`from_str_with_options`].
pub fn parse_with_options<'a, S>(input: &'a str, options: &Options<S>) -> MarkdownContent<'a>
where
    S: StyleSheet,
{
    let mut parse_opts = ParseOptions::empty();
    parse_opts.insert(ParseOptions::ENABLE_STRIKETHROUGH);
    parse_opts.insert(ParseOptions::ENABLE_TASKLISTS);
    parse_opts.insert(ParseOptions::ENABLE_HEADING_ATTRIBUTES);
    parse_opts.insert(ParseOptions::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    parse_opts.insert(ParseOptions::ENABLE_SUPERSCRIPT);
    parse_opts.insert(ParseOptions::ENABLE_SUBSCRIPT);
    parse_opts.insert(ParseOptions::ENABLE_TABLES);
    parse_opts.insert(ParseOptions::ENABLE_GFM);
    parse_opts.insert(ParseOptions::ENABLE_FOOTNOTES);
    parse_opts.insert(ParseOptions::ENABLE_MATH);
    parse_opts.insert(ParseOptions::ENABLE_DEFINITION_LIST);
    let parser = Parser::new_ext(input, parse_opts);

    let mut writer = TextWriter::with_image_blocks(
        parser,
        options.styles.clone(),
        options.code_theme.clone(),
        #[cfg(feature = "highlight-code")]
        options.code_theme_override.clone(),
    );
    writer.run();
    writer.into_content()
}

/// Returns the names of all built-in syntax highlighting themes.
///
/// These names can be passed to [`Options::code_theme`].
///
/// Only available when the `highlight-code` feature is enabled.
#[cfg(feature = "highlight-code")]
pub fn available_themes() -> Vec<&'static str> {
    THEME_SET.themes.keys().map(|s| s.as_str()).collect()
}

// Heading attributes collected from pulldown-cmark to render after the heading text.
struct HeadingMeta<'a> {
    id: Option<CowStr<'a>>,
    classes: Vec<CowStr<'a>>,
    attrs: Vec<(CowStr<'a>, Option<CowStr<'a>>)>,
}

impl<'a> HeadingMeta<'a> {
    fn into_option(self) -> Option<Self> {
        let has_id = self.id.is_some();
        let has_classes = !self.classes.is_empty();
        let has_attrs = !self.attrs.is_empty();
        if has_id || has_classes || has_attrs {
            Some(self)
        } else {
            None
        }
    }

    // Format as a Markdown attribute block suffix, e.g. "{#id .class key=value}".
    fn to_suffix(&self) -> Option<String> {
        let mut parts = Vec::new();

        if let Some(id) = &self.id {
            parts.push(format!("#{}", id));
        }

        for class in &self.classes {
            parts.push(format!(".{}", class));
        }

        for (key, value) in &self.attrs {
            match value {
                Some(value) => parts.push(format!("{}={}", key, value)),
                None => parts.push(key.to_string()),
            }
        }

        if parts.is_empty() {
            None
        } else {
            Some(format!(" {{{}}}", parts.join(" ")))
        }
    }
}

struct TextWriter<'a, I, S: StyleSheet> {
    /// Iterator supplying events.
    iter: I,

    /// Text to write to.
    text: Text<'a>,

    /// Current style.
    ///
    /// This is a stack of styles, with the top style being the current style.
    inline_styles: Vec<Style>,

    /// Prefix to add to the start of the each line.
    line_prefixes: Vec<Span<'a>>,

    /// Stack of line styles.
    line_styles: Vec<Style>,

    /// Used to highlight code blocks, set when  a codeblock is encountered
    #[cfg(feature = "highlight-code")]
    code_highlighter: Option<HighlightLines<'a>>,

    /// Current list index as a stack of indices.
    list_indices: Vec<Option<u64>>,

    /// A link which will be appended to the current line when the link tag is closed.
    link: Option<CowStr<'a>>,

    /// Image destination URL stored between Start(Image) and End(Image).
    image_url: Option<CowStr<'a>>,

    /// Whether any text was rendered between Start(Image) and End(Image).
    image_had_alt: bool,

    /// The [`StyleSheet`] to use to style the output.
    styles: S,

    /// Heading attributes to append after heading content.
    heading_meta: Option<HeadingMeta<'a>>,

    /// Whether we are inside a metadata block.
    in_metadata_block: bool,

    /// Active table builder that accumulates cells during table parsing.
    table_builder: Option<tables::TableBuilder<'a>>,

    /// Whether we are inside a definition list.
    in_definition_list: bool,

    /// Whether we are inside a footnote definition (suppresses paragraph newline).
    in_footnote_definition: bool,

    /// Current line number inside a code block (Some(n) when inside, None otherwise).
    code_line_number: Option<u32>,

    /// Whether to emit image blocks instead of inline alt text.
    emit_image_blocks: bool,

    /// Accumulated content blocks when `emit_image_blocks` is true.
    content_blocks: Vec<MarkdownBlock<'a>>,

    /// Whether we are collecting alt text for an image block.
    collecting_image_alt: bool,

    /// Buffer for collecting image alt text.
    image_alt_buffer: String,

    /// Name of the syntect theme for code highlighting.
    #[cfg_attr(not(feature = "highlight-code"), allow(dead_code))]
    code_theme_name: String,

    /// Optional custom theme that takes precedence over `code_theme_name`.
    /// Stored as a leaked `&'static` reference so it satisfies the `HighlightLines<'a>` borrow.
    #[cfg(feature = "highlight-code")]
    code_theme_override: Option<&'static syntect::highlighting::Theme>,

    needs_newline: bool,
}

#[cfg(feature = "highlight-code")]
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
#[cfg(feature = "highlight-code")]
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

impl<'a, I, S> TextWriter<'a, I, S>
where
    I: Iterator<Item = Event<'a>>,
    S: StyleSheet,
{
    fn new(
        iter: I,
        styles: S,
        code_theme_name: String,
        #[cfg(feature = "highlight-code")] code_theme_override: Option<
            syntect::highlighting::Theme,
        >,
    ) -> Self {
        Self::create(
            iter,
            styles,
            false,
            code_theme_name,
            #[cfg(feature = "highlight-code")]
            code_theme_override,
        )
    }

    fn with_image_blocks(
        iter: I,
        styles: S,
        code_theme_name: String,
        #[cfg(feature = "highlight-code")] code_theme_override: Option<
            syntect::highlighting::Theme,
        >,
    ) -> Self {
        Self::create(
            iter,
            styles,
            true,
            code_theme_name,
            #[cfg(feature = "highlight-code")]
            code_theme_override,
        )
    }

    fn create(
        iter: I,
        styles: S,
        emit_image_blocks: bool,
        code_theme_name: String,
        #[cfg(feature = "highlight-code")] code_theme_override: Option<
            syntect::highlighting::Theme,
        >,
    ) -> Self {
        // Leak the custom theme into a &'static reference so it can satisfy
        // the HighlightLines<'a> borrow requirement. This is intentional —
        // themes are small and typically live for the program's duration.
        #[cfg(feature = "highlight-code")]
        let code_theme_override =
            code_theme_override.map(|t| &*Box::leak(Box::new(t)));
        Self {
            iter,
            text: Text::default(),
            inline_styles: vec![],
            line_styles: vec![],
            line_prefixes: vec![],
            list_indices: vec![],
            needs_newline: false,
            #[cfg(feature = "highlight-code")]
            code_highlighter: None,
            link: None,
            image_url: None,
            image_had_alt: false,
            styles,
            heading_meta: None,
            in_metadata_block: false,
            table_builder: None,
            in_definition_list: false,
            in_footnote_definition: false,
            code_line_number: None,
            emit_image_blocks,
            content_blocks: Vec::new(),
            collecting_image_alt: false,
            image_alt_buffer: String::new(),
            code_theme_name,
            #[cfg(feature = "highlight-code")]
            code_theme_override,
        }
    }

    fn run(&mut self) {
        debug!("Running text writer");
        while let Some(event) = self.iter.next() {
            self.handle_event(event);
        }
        if self.emit_image_blocks {
            self.flush_text_block();
        }
    }

    /// Flush the current accumulated text into a `MarkdownBlock::Text` block.
    fn flush_text_block(&mut self) {
        let text = std::mem::take(&mut self.text);
        if !text.lines.is_empty() {
            self.content_blocks.push(MarkdownBlock::Text(text));
        }
    }

    /// Consume the writer and return a [`MarkdownContent`] with all accumulated blocks.
    fn into_content(self) -> MarkdownContent<'a> {
        MarkdownContent {
            blocks: self.content_blocks,
        }
    }

    #[instrument(level = "debug", skip(self))]
    fn handle_event(&mut self, event: Event<'a>) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.text(text),
            Event::Code(code) => self.code(code),
            Event::Html(html) => self.html_block(html),
            Event::InlineHtml(html) => self.inline_html(html),
            Event::FootnoteReference(label) => self.footnote_reference(label),
            Event::SoftBreak => self.soft_break(),
            Event::HardBreak => self.hard_break(),
            Event::Rule => self.rule(),
            Event::TaskListMarker(checked) => self.task_list_marker(checked),
            Event::InlineMath(math) => self.inline_math(math),
            Event::DisplayMath(math) => self.display_math(math),
        }
    }

    fn start_tag(&mut self, tag: Tag<'a>) {
        match tag {
            Tag::Paragraph => self.start_paragraph(),
            Tag::Heading {
                level,
                id,
                classes,
                attrs,
            } => self.start_heading(level, HeadingMeta { id, classes, attrs }),
            Tag::BlockQuote(kind) => self.start_blockquote(kind),
            Tag::CodeBlock(kind) => self.start_codeblock(kind),
            Tag::HtmlBlock => self.start_html_block(),
            Tag::List(start_index) => self.start_list(start_index),
            Tag::Item => self.start_item(),
            Tag::FootnoteDefinition(label) => self.start_footnote_definition(label),
            Tag::Table(alignments) => self.start_table(alignments),
            Tag::TableHead => self.start_table_row(),
            Tag::TableRow => self.start_table_row(),
            Tag::TableCell => self.start_table_cell(),
            Tag::Emphasis => self.push_inline_style(Style::new().italic()),
            Tag::Strong => self.push_inline_style(Style::new().bold()),
            Tag::Strikethrough => self.push_inline_style(Style::new().crossed_out()),
            Tag::Subscript => self.push_inline_style(Style::new().dim().italic()),
            Tag::Superscript => self.push_inline_style(Style::new().dim().italic()),
            Tag::Link { dest_url, .. } => self.push_link(dest_url),
            Tag::Image { dest_url, .. } => self.start_image(dest_url),
            Tag::MetadataBlock(_) => self.start_metadata_block(),
            Tag::DefinitionList => self.start_definition_list(),
            Tag::DefinitionListTitle => self.start_definition_title(),
            Tag::DefinitionListDefinition => self.start_definition_desc(),
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.end_paragraph(),
            TagEnd::Heading(_) => self.end_heading(),
            TagEnd::BlockQuote(_) => self.end_blockquote(),
            TagEnd::CodeBlock => self.end_codeblock(),
            TagEnd::HtmlBlock => self.end_html_block(),
            TagEnd::List(_is_ordered) => self.end_list(),
            TagEnd::Item => {}
            TagEnd::FootnoteDefinition => self.end_footnote_definition(),
            TagEnd::Table => self.end_table(),
            TagEnd::TableHead => self.end_table_head(),
            TagEnd::TableRow => self.end_table_row(),
            TagEnd::TableCell => self.end_table_cell(),
            TagEnd::Emphasis => self.pop_inline_style(),
            TagEnd::Strong => self.pop_inline_style(),
            TagEnd::Strikethrough => self.pop_inline_style(),
            TagEnd::Subscript => self.pop_inline_style(),
            TagEnd::Superscript => self.pop_inline_style(),
            TagEnd::Link => self.pop_link(),
            TagEnd::Image => self.end_image(),
            TagEnd::MetadataBlock(_) => self.end_metadata_block(),
            TagEnd::DefinitionList => self.end_definition_list(),
            TagEnd::DefinitionListTitle => self.end_definition_title(),
            TagEnd::DefinitionListDefinition => self.end_definition_desc(),
        }
    }

    fn start_paragraph(&mut self) {
        // Inside a footnote definition, content should flow on the same line as [label]:
        if self.in_footnote_definition {
            return;
        }
        // Insert an empty line between paragraphs if there is at least one line of text already.
        if self.needs_newline {
            self.push_line(Line::default());
        }
        self.push_line(Line::default());
        self.needs_newline = false;
    }

    fn end_paragraph(&mut self) {
        self.needs_newline = true
    }

    fn start_heading(&mut self, level: HeadingLevel, heading_meta: HeadingMeta<'a>) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        let heading_level = match level {
            HeadingLevel::H1 => 1,
            HeadingLevel::H2 => 2,
            HeadingLevel::H3 => 3,
            HeadingLevel::H4 => 4,
            HeadingLevel::H5 => 5,
            HeadingLevel::H6 => 6,
        };
        let style = self.styles.heading(heading_level);
        let content = format!("{} ", "#".repeat(heading_level as usize));
        self.push_line(Line::styled(content, style));
        self.heading_meta = heading_meta.into_option();
        self.needs_newline = false;
    }

    fn end_heading(&mut self) {
        if let Some(meta) = self.heading_meta.take() {
            if let Some(suffix) = meta.to_suffix() {
                self.push_span(Span::styled(suffix, self.styles.heading_meta()));
            }
        }
        self.needs_newline = true
    }

    fn start_blockquote(&mut self, kind: Option<BlockQuoteKind>) {
        if self.needs_newline {
            self.push_line(Line::default());
            self.needs_newline = false;
        }

        if let Some(alert_kind) = kind {
            let (icon, kind_str) = match alert_kind {
                BlockQuoteKind::Note => ("\u{2139}\u{FE0F}", "note"),
                BlockQuoteKind::Tip => ("\u{1F4A1}", "tip"),
                BlockQuoteKind::Important => ("\u{2757}", "important"),
                BlockQuoteKind::Warning => ("\u{26A0}\u{FE0F}", "warning"),
                BlockQuoteKind::Caution => ("\u{1F534}", "caution"),
            };
            let style = self.styles.alert(kind_str);
            self.line_prefixes.push(Span::from("\u{2502}"));
            self.line_styles.push(style);
            // Render the alert header line.
            let label = kind_str[..1].to_uppercase() + &kind_str[1..];
            self.push_line(Line::default());
            self.push_span(Span::styled(
                format!("{icon} {label}"),
                style.patch(Style::new().bold()),
            ));
            self.needs_newline = false;
        } else {
            self.line_prefixes.push(Span::from("\u{2502}"));
            self.line_styles.push(self.styles.blockquote());
        }
    }

    fn end_blockquote(&mut self) {
        self.line_prefixes.pop();
        self.line_styles.pop();
        self.needs_newline = true;
    }

    fn text(&mut self, text: CowStr<'a>) {
        // Redirect to table builder if active.
        if let Some(builder) = &mut self.table_builder {
            let style = self.inline_styles.last().copied().unwrap_or_default();
            builder.push_span(Span::styled(text.into_string(), style));
            return;
        }

        // Capture alt text for image blocks instead of rendering.
        if self.collecting_image_alt {
            self.image_alt_buffer.push_str(&text);
            return;
        }

        // Track that we received alt text while inside an image.
        if self.image_url.is_some() {
            self.image_had_alt = true;
        }

        #[cfg(feature = "highlight-code")]
        if let Some(highlighter) = &mut self.code_highlighter {
            let text: Text = LinesWithEndings::from(&text)
                .filter_map(|line| highlighter.highlight_line(line, &SYNTAX_SET).ok())
                .filter_map(|part| as_24_bit_terminal_escaped(&part, false).into_text().ok())
                .flatten()
                .collect();

            for line in text.lines {
                if let Some(num) = &mut self.code_line_number {
                    *num += 1;
                    let gutter_style = self.styles.code_line_number();
                    let gutter = Span::styled(format!("{:>3} \u{2502} ", num), gutter_style);
                    let mut new_line = Line::default();
                    new_line.push_span(gutter);
                    for span in line.spans {
                        new_line.push_span(span);
                    }
                    self.text.push_line(new_line);
                } else {
                    self.text.push_line(line);
                }
            }
            self.needs_newline = false;
            return;
        }

        for (position, line) in text.lines().with_position() {
            if self.needs_newline {
                self.push_line(Line::default());
                self.needs_newline = false;
            }
            if matches!(position, Position::Middle | Position::Last) {
                self.push_line(Line::default());
            }

            // Add line number gutter for code blocks.
            if let Some(num) = &mut self.code_line_number {
                *num += 1;
                let gutter_style = self.styles.code_line_number();
                let gutter = Span::styled(format!("{:>3} \u{2502} ", num), gutter_style);
                self.push_span(gutter);
            }

            let style = self.inline_styles.last().copied().unwrap_or_default();

            let span = Span::styled(line.to_owned(), style);

            self.push_span(span);
        }
        self.needs_newline = false;
    }

    fn code(&mut self, code: CowStr<'a>) {
        // Redirect to table builder if active.
        if let Some(builder) = &mut self.table_builder {
            builder.push_span(Span::styled(code.into_string(), self.styles.code()));
            return;
        }
        let span = Span::styled(code, self.styles.code());
        self.push_span(span);
    }

    fn hard_break(&mut self) {
        self.push_line(Line::default());
    }

    fn start_metadata_block(&mut self) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        self.line_styles.push(self.styles.metadata_block());
        self.push_line(Line::from("---"));
        self.push_line(Line::default());
        self.in_metadata_block = true;
    }

    fn end_metadata_block(&mut self) {
        if self.in_metadata_block {
            self.push_line(Line::from("---"));
            self.line_styles.pop();
            self.in_metadata_block = false;
            self.needs_newline = true;
        }
    }

    fn rule(&mut self) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        let rule_line = "\u{2500}".repeat(40);
        self.push_line(Line::styled(rule_line, self.styles.rule()));
        self.needs_newline = true;
    }

    fn start_list(&mut self, index: Option<u64>) {
        if self.list_indices.is_empty() && self.needs_newline {
            self.push_line(Line::default());
        }
        self.list_indices.push(index);
    }

    fn end_list(&mut self) {
        self.list_indices.pop();
        self.needs_newline = true;
    }

    fn start_item(&mut self) {
        self.push_line(Line::default());
        let width = self.list_indices.len() * 4 - 3;
        if let Some(last_index) = self.list_indices.last_mut() {
            let span = match last_index {
                None => Span::from(" ".repeat(width - 1) + "- "),
                Some(index) => {
                    *index += 1;
                    format!("{:width$}. ", *index - 1).light_blue()
                }
            };
            self.push_span(span);
        }
        self.needs_newline = false;
    }

    fn task_list_marker(&mut self, checked: bool) {
        let marker = if checked { 'x' } else { ' ' };
        let marker_span = Span::from(format!("[{}] ", marker));
        if let Some(line) = self.text.lines.last_mut() {
            if let Some(first_span) = line.spans.first_mut() {
                let content = first_span.content.to_mut();
                if content.ends_with("- ") {
                    let len = content.len();
                    content.truncate(len - 2);
                    content.push_str("- [");
                    content.push(marker);
                    content.push_str("] ");
                    return;
                }
            }
            line.spans.insert(1, marker_span);
        } else {
            self.push_span(marker_span);
        }
    }

    fn soft_break(&mut self) {
        if self.in_metadata_block {
            self.hard_break();
        } else {
            self.push_span(Span::raw(" "));
        }
    }

    fn start_codeblock(&mut self, kind: CodeBlockKind<'_>) {
        if !self.text.lines.is_empty() {
            self.push_line(Line::default());
        }
        let lang = match kind {
            CodeBlockKind::Fenced(ref lang) => lang.as_ref(),
            CodeBlockKind::Indented => "",
        };

        #[cfg(not(feature = "highlight-code"))]
        self.line_styles.push(self.styles.code());

        #[cfg(feature = "highlight-code")]
        self.set_code_highlighter(lang);

        // Opening fence: dim style
        let fence_style = self.styles.code_fence();
        let span = Span::styled(format!("```{lang}"), fence_style);
        self.push_line(span.into());

        // Start line numbering
        self.code_line_number = Some(0);
        self.needs_newline = true;
    }

    fn end_codeblock(&mut self) {
        // Stop line numbering before closing fence
        self.code_line_number = None;

        // Closing fence: dim style
        let fence_style = self.styles.code_fence();
        let span = Span::styled("```", fence_style);
        self.push_line(span.into());
        self.needs_newline = true;

        #[cfg(not(feature = "highlight-code"))]
        self.line_styles.pop();

        #[cfg(feature = "highlight-code")]
        self.clear_code_highlighter();
    }

    #[cfg(feature = "highlight-code")]
    #[instrument(level = "trace", skip(self))]
    fn set_code_highlighter(&mut self, lang: &str) {
        if let Some(syntax) = SYNTAX_SET.find_syntax_by_token(lang) {
            debug!("Starting code block with syntax: {:?}", lang);
            let theme: &syntect::highlighting::Theme =
                if let Some(custom) = &self.code_theme_override {
                    custom
                } else {
                    THEME_SET
                        .themes
                        .get(&self.code_theme_name)
                        .unwrap_or_else(|| {
                            warn!(
                                "Theme {:?} not found, falling back to {:?}",
                                self.code_theme_name,
                                Options::<DefaultStyleSheet>::DEFAULT_CODE_THEME,
                            );
                            &THEME_SET.themes[Options::<DefaultStyleSheet>::DEFAULT_CODE_THEME]
                        })
                };
            let highlighter = HighlightLines::new(syntax, theme);
            self.code_highlighter = Some(highlighter);
        } else {
            warn!("Could not find syntax for code block: {:?}", lang);
        }
    }

    #[cfg(feature = "highlight-code")]
    #[instrument(level = "trace", skip(self))]
    fn clear_code_highlighter(&mut self) {
        self.code_highlighter = None;
    }

    #[instrument(level = "trace", skip(self))]
    fn push_inline_style(&mut self, style: Style) {
        let current_style = self.inline_styles.last().copied().unwrap_or_default();
        let style = current_style.patch(style);
        self.inline_styles.push(style);
        debug!("Pushed inline style: {:?}", style);
        debug!("Current inline styles: {:?}", self.inline_styles);
    }

    #[instrument(level = "trace", skip(self))]
    fn pop_inline_style(&mut self) {
        self.inline_styles.pop();
    }

    #[instrument(level = "trace", skip(self))]
    fn push_line(&mut self, line: Line<'a>) {
        let style = self.line_styles.last().copied().unwrap_or_default();
        let mut line = line.patch_style(style);

        // Add line prefixes to the start of the line.
        let line_prefixes = self.line_prefixes.iter().cloned().collect_vec();
        let has_prefixes = !line_prefixes.is_empty();
        if has_prefixes {
            line.spans.insert(0, " ".into());
        }
        for prefix in line_prefixes.iter().rev().cloned() {
            line.spans.insert(0, prefix);
        }
        self.text.lines.push(line);
    }

    #[instrument(level = "trace", skip(self))]
    fn push_span(&mut self, span: Span<'a>) {
        if let Some(line) = self.text.lines.last_mut() {
            line.push_span(span);
        } else {
            self.push_line(Line::from(vec![span]));
        }
    }

    /// Store the link and push the link style so the link text is also styled.
    #[instrument(level = "trace", skip(self))]
    fn push_link(&mut self, dest_url: CowStr<'a>) {
        self.link = Some(dest_url);
        self.push_inline_style(self.styles.link());
    }

    /// Pop the link style and append the link URL to the current line.
    #[instrument(level = "trace", skip(self))]
    fn pop_link(&mut self) {
        self.pop_inline_style();
        if let Some(link) = self.link.take() {
            self.push_span(" (".into());
            self.push_span(Span::styled(link, self.styles.link()));
            self.push_span(")".into());
        }
    }

    fn start_table(&mut self, alignments: Vec<pulldown_cmark::Alignment>) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        self.table_builder = Some(tables::TableBuilder::new(alignments));
        self.needs_newline = false;
    }

    fn start_table_row(&mut self) {
        if let Some(builder) = &mut self.table_builder {
            builder.start_row();
        }
    }

    fn start_table_cell(&mut self) {
        if let Some(builder) = &mut self.table_builder {
            builder.start_cell();
        }
    }

    fn end_table_cell(&mut self) {
        if let Some(builder) = &mut self.table_builder {
            builder.finish_cell();
        }
    }

    fn end_table_row(&mut self) {
        if let Some(builder) = &mut self.table_builder {
            builder.finish_row();
        }
    }

    fn end_table_head(&mut self) {
        if let Some(builder) = &mut self.table_builder {
            builder.finish_row();
            builder.finish_header();
        }
    }

    fn end_table(&mut self) {
        if let Some(builder) = self.table_builder.take() {
            let lines = builder.render(&self.styles);
            for line in lines {
                self.push_line(line);
            }
            self.needs_newline = true;
        }
    }

    // --- HTML handling ---

    fn start_html_block(&mut self) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        self.push_line(Line::default());
        self.line_styles.push(self.styles.html());
        self.needs_newline = false;
    }

    fn end_html_block(&mut self) {
        self.line_styles.pop();
        self.needs_newline = true;
    }

    fn html_block(&mut self, html: CowStr<'a>) {
        let style = self.styles.html();
        for line in html.lines() {
            if self.needs_newline {
                self.push_line(Line::default());
                self.needs_newline = false;
            }
            self.push_span(Span::styled(line.to_owned(), style));
            self.needs_newline = true;
        }
    }

    fn inline_html(&mut self, html: CowStr<'a>) {
        let style = self.styles.html();
        self.push_span(Span::styled(html, style));
    }

    // --- Footnotes ---

    fn footnote_reference(&mut self, label: CowStr<'a>) {
        let style = self.styles.footnote_ref();
        self.push_span(Span::styled(format!("[{label}]"), style));
    }

    fn start_footnote_definition(&mut self, label: CowStr<'a>) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        let style = self.styles.footnote_def();
        self.push_line(Line::default());
        self.push_span(Span::styled(format!("[{label}]: "), style));
        self.line_styles.push(style);
        self.in_footnote_definition = true;
        self.needs_newline = false;
    }

    fn end_footnote_definition(&mut self) {
        self.line_styles.pop();
        self.in_footnote_definition = false;
        self.needs_newline = true;
    }

    // --- Math ---

    fn inline_math(&mut self, math: CowStr<'a>) {
        let delim_style = Style::new().dark_gray();
        let content_style = self.styles.math_inline();
        self.push_span(Span::styled("$", delim_style));
        self.push_span(Span::styled(math, content_style));
        self.push_span(Span::styled("$", delim_style));
    }

    fn display_math(&mut self, math: CowStr<'a>) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        let content_style = self.styles.math_display();
        self.push_line(Line::default());
        self.push_span(Span::styled("  ", Style::default()));
        self.push_span(Span::styled(math, content_style));
        self.needs_newline = true;
    }

    // --- Definition Lists ---

    fn start_definition_list(&mut self) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        self.in_definition_list = true;
        self.needs_newline = false;
    }

    fn end_definition_list(&mut self) {
        self.in_definition_list = false;
        self.needs_newline = true;
    }

    fn start_definition_title(&mut self) {
        self.push_line(Line::default());
        self.push_inline_style(self.styles.definition_title());
        self.needs_newline = false;
    }

    fn end_definition_title(&mut self) {
        self.pop_inline_style();
        self.needs_newline = false;
    }

    fn start_definition_desc(&mut self) {
        self.push_line(Line::default());
        self.push_span(Span::styled(": ", self.styles.definition_desc()));
        self.push_inline_style(self.styles.definition_desc());
        self.needs_newline = false;
    }

    fn end_definition_desc(&mut self) {
        self.pop_inline_style();
        self.needs_newline = false;
    }

    /// Store the image URL and push the image alt style.
    #[instrument(level = "trace", skip(self))]
    fn start_image(&mut self, dest_url: CowStr<'a>) {
        self.image_url = Some(dest_url);
        self.image_had_alt = false;
        if self.emit_image_blocks {
            self.collecting_image_alt = true;
            self.image_alt_buffer.clear();
        } else {
            self.push_inline_style(self.styles.image_alt());
        }
    }

    /// Render the image as alt-text fallback, or emit an image block.
    #[instrument(level = "trace", skip(self))]
    fn end_image(&mut self) {
        if self.emit_image_blocks {
            self.collecting_image_alt = false;
            if let Some(url) = self.image_url.take() {
                // Flush accumulated text before the image block.
                self.flush_text_block();
                self.content_blocks.push(MarkdownBlock::Image {
                    url: url.into_string(),
                    alt: std::mem::take(&mut self.image_alt_buffer),
                    title: None,
                });
            }
            self.image_had_alt = false;
            return;
        }

        self.pop_inline_style();
        if let Some(url) = self.image_url.take() {
            let style = self.styles.image_alt();
            let prefix = format!("{} ", images::IMAGE_INDICATOR);
            if self.image_had_alt {
                // Alt text was already rendered by text(). Prepend the image indicator.
                if let Some(line) = self.text.lines.last_mut() {
                    let mut found = false;
                    for span in &mut line.spans {
                        if span.style == style {
                            let content = span.content.to_mut();
                            content.insert_str(0, &prefix);
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        line.spans.insert(0, Span::styled(prefix, style));
                    }
                }
            } else {
                // No alt text, render the URL as fallback.
                let content = format!("{prefix}{url}");
                self.push_span(Span::styled(content, style));
            }
        }
        self.image_had_alt = false;
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};
    use tracing::level_filters::LevelFilter;
    use tracing::subscriber::{self, DefaultGuard};
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::fmt::time::Uptime;

    use super::*;

    #[fixture]
    fn with_tracing() -> DefaultGuard {
        let subscriber = tracing_subscriber::fmt()
            .with_test_writer()
            .with_timer(Uptime::default())
            .with_max_level(LevelFilter::TRACE)
            .with_span_events(FmtSpan::ENTER)
            .finish();
        subscriber::set_default(subscriber)
    }

    #[rstest]
    fn empty(_with_tracing: DefaultGuard) {
        assert_eq!(from_str(""), Text::default());
    }

    #[rstest]
    fn paragraph_single(_with_tracing: DefaultGuard) {
        assert_eq!(from_str("Hello, world!"), Text::from("Hello, world!"));
    }

    #[rstest]
    fn paragraph_soft_break(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str(indoc! {"
                Hello
                World
            "}),
            Text::from(Line::from_iter([
                Span::from("Hello"),
                Span::from(" "),
                Span::from("World"),
            ]))
        );
    }

    #[rstest]
    fn paragraph_multiple(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str(indoc! {"
                Paragraph 1
                
                Paragraph 2
            "}),
            Text::from_iter(["Paragraph 1", "", "Paragraph 2",])
        );
    }

    #[rstest]
    fn rule(_with_tracing: DefaultGuard) {
        let rule_str = "\u{2500}".repeat(40);
        assert_eq!(
            from_str(indoc! {"
                Paragraph 1

                ---

                Paragraph 2
            "}),
            Text::from_iter([
                Line::from("Paragraph 1"),
                Line::default(),
                Line::styled(rule_str, Style::new().dark_gray()),
                Line::default(),
                Line::from("Paragraph 2"),
            ])
        );
    }

    #[rstest]
    fn headings(_with_tracing: DefaultGuard) {
        let h1 = Style::new().on_cyan().bold().underlined();
        let h2 = Style::new().cyan().bold();
        let h3 = Style::new().cyan().bold().italic();
        let h4 = Style::new().light_cyan().italic();
        let h5 = Style::new().light_cyan().italic();
        let h6 = Style::new().light_cyan().italic();

        assert_eq!(
            from_str(indoc! {"
                # Heading 1
                ## Heading 2
                ### Heading 3
                #### Heading 4
                ##### Heading 5
                ###### Heading 6
            "}),
            Text::from_iter([
                Line::from_iter(["# ", "Heading 1"]).style(h1),
                Line::default(),
                Line::from_iter(["## ", "Heading 2"]).style(h2),
                Line::default(),
                Line::from_iter(["### ", "Heading 3"]).style(h3),
                Line::default(),
                Line::from_iter(["#### ", "Heading 4"]).style(h4),
                Line::default(),
                Line::from_iter(["##### ", "Heading 5"]).style(h5),
                Line::default(),
                Line::from_iter(["###### ", "Heading 6"]).style(h6),
            ])
        );
    }

    #[rstest]
    fn heading_attributes(_with_tracing: DefaultGuard) {
        let h1 = Style::new().on_cyan().bold().underlined();
        let meta = Style::new().dim();

        assert_eq!(
            from_str("# Heading {#title .primary data-kind=doc}"),
            Text::from(
                Line::from_iter([
                    Span::from("# "),
                    Span::from("Heading"),
                    Span::styled(" {#title .primary data-kind=doc}", meta),
                ])
                .style(h1)
            )
        );
    }

    mod blockquote {
        use pretty_assertions::assert_eq;
        use ratatui::style::Color;

        use super::*;

        const STYLE: Style = Style::new().fg(Color::Green);

        /// I was having difficulty getting the right number of newlines between paragraphs, so this
        /// test is to help debug and ensure that.
        #[rstest]
        fn after_paragraph(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str(indoc! {"
                Hello, world!

                > Blockquote
            "}),
                Text::from_iter([
                    Line::from("Hello, world!"),
                    Line::default(),
                    Line::from_iter(["\u{2502}", " ", "Blockquote"]).style(STYLE),
                ])
            );
        }
        #[rstest]
        fn single(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("> Blockquote"),
                Text::from(Line::from_iter(["\u{2502}", " ", "Blockquote"]).style(STYLE))
            );
        }

        #[rstest]
        fn soft_break(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str(indoc! {"
                > Blockquote 1
                > Blockquote 2
            "}),
                Text::from(
                    Line::from_iter(["\u{2502}", " ", "Blockquote 1", " ", "Blockquote 2"])
                        .style(STYLE),
                )
            );
        }

        #[rstest]
        fn multiple(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str(indoc! {"
                > Blockquote 1
                >
                > Blockquote 2
            "}),
                Text::from_iter([
                    Line::from_iter(["\u{2502}", " ", "Blockquote 1"]).style(STYLE),
                    Line::from_iter(["\u{2502}", " "]).style(STYLE),
                    Line::from_iter(["\u{2502}", " ", "Blockquote 2"]).style(STYLE),
                ])
            );
        }

        #[rstest]
        fn multiple_with_break(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str(indoc! {"
                > Blockquote 1

                > Blockquote 2
            "}),
                Text::from_iter([
                    Line::from_iter(["\u{2502}", " ", "Blockquote 1"]).style(STYLE),
                    Line::default(),
                    Line::from_iter(["\u{2502}", " ", "Blockquote 2"]).style(STYLE),
                ])
            );
        }

        #[rstest]
        fn nested(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str(indoc! {"
                > Blockquote 1
                >> Nested Blockquote
            "}),
                Text::from_iter([
                    Line::from_iter(["\u{2502}", " ", "Blockquote 1"]).style(STYLE),
                    Line::from_iter(["\u{2502}", " "]).style(STYLE),
                    Line::from_iter(["\u{2502}", "\u{2502}", " ", "Nested Blockquote"])
                        .style(STYLE),
                ])
            );
        }
    }

    #[rstest]
    fn list_single(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str(indoc! {"
                - List item 1
            "}),
            Text::from_iter([Line::from_iter(["- ", "List item 1"])])
        );
    }

    #[rstest]
    fn list_multiple(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str(indoc! {"
                - List item 1
                - List item 2
            "}),
            Text::from_iter([
                Line::from_iter(["- ", "List item 1"]),
                Line::from_iter(["- ", "List item 2"]),
            ])
        );
    }

    #[rstest]
    fn list_ordered(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str(indoc! {"
                1. List item 1
                2. List item 2
            "}),
            Text::from_iter([
                Line::from_iter(["1. ".light_blue(), "List item 1".into()]),
                Line::from_iter(["2. ".light_blue(), "List item 2".into()]),
            ])
        );
    }

    #[rstest]
    fn list_nested(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str(indoc! {"
                - List item 1
                  - Nested list item 1
            "}),
            Text::from_iter([
                Line::from_iter(["- ", "List item 1"]),
                Line::from_iter(["    - ", "Nested list item 1"]),
            ])
        );
    }

    #[rstest]
    fn list_task_items(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str(indoc! {"
                - [ ] Incomplete
                - [x] Complete
            "}),
            Text::from_iter([
                Line::from_iter(["- [ ] ", "Incomplete"]),
                Line::from_iter(["- [x] ", "Complete"]),
            ])
        );
    }

    #[rstest]
    fn list_task_items_ordered(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str(indoc! {"
                1. [ ] Incomplete
                2. [x] Complete
            "}),
            Text::from_iter([
                Line::from_iter(["1. ".light_blue(), "[ ] ".into(), "Incomplete".into(),]),
                Line::from_iter(["2. ".light_blue(), "[x] ".into(), "Complete".into(),]),
            ])
        );
    }

    #[cfg_attr(not(feature = "highlight-code"), ignore)]
    #[rstest]
    fn highlighted_code(_with_tracing: DefaultGuard) {
        // Assert no extra newlines are added
        let highlighted_code = from_str(indoc! {"
            ```rust
            fn main() {
                println!(\"Hello, highlighted code!\");
            }
            ```"});

        insta::assert_snapshot!(highlighted_code);
        insta::assert_debug_snapshot!(highlighted_code);
    }

    #[cfg_attr(not(feature = "highlight-code"), ignore)]
    #[rstest]
    fn highlighted_code_with_indentation(_with_tracing: DefaultGuard) {
        // Assert no extra newlines are added
        let highlighted_code_indented = from_str(indoc! {"
            ```rust
            fn main() {
                // This is a comment
                HelloWorldBuilder::new()
                    .with_text(\"Hello, highlighted code!\")
                    .build()
                    .show();
                            
            }
            ```"});

        insta::assert_snapshot!(highlighted_code_indented);
        insta::assert_debug_snapshot!(highlighted_code_indented);
    }

    #[cfg_attr(feature = "highlight-code", ignore)]
    #[rstest]
    fn unhighlighted_code(_with_tracing: DefaultGuard) {
        // Assert no extra newlines are added
        let unhiglighted_code = from_str(indoc! {"
            ```rust
            fn main() {
                println!(\"Hello, unhighlighted code!\");
            }
            ```"});

        insta::assert_snapshot!(unhiglighted_code);

        // Code highlighting is complex, assert on on the debug snapshot
        insta::assert_debug_snapshot!(unhiglighted_code);
    }

    #[rstest]
    fn inline_code(_with_tracing: DefaultGuard) {
        let text = from_str("Example of `Inline code`");
        insta::assert_snapshot!(text);

        assert_eq!(
            text,
            Line::from_iter([
                Span::from("Example of "),
                Span::styled("Inline code", Style::new().white().on_black())
            ])
            .into()
        );
    }

    #[rstest]
    fn superscript(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str("H ^2^ O"),
            Text::from(Line::from_iter([
                Span::from("H "),
                Span::styled("2", Style::new().dim().italic()),
                Span::from(" O"),
            ]))
        );
    }

    #[rstest]
    fn subscript(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str("H ~2~ O"),
            Text::from(Line::from_iter([
                Span::from("H "),
                Span::styled("2", Style::new().dim().italic()),
                Span::from(" O"),
            ]))
        );
    }

    #[rstest]
    fn metadata_block(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str(indoc! {"
                ---
                title: Demo
                ---

                Body
            "}),
            Text::from_iter([
                Line::from("---").style(Style::new().light_yellow()),
                Line::from("title: Demo").style(Style::new().light_yellow()),
                Line::from("---").style(Style::new().light_yellow()),
                Line::default(),
                Line::from("Body"),
            ])
        );
    }

    #[rstest]
    fn strong(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str("**Strong**"),
            Text::from(Line::from("Strong".bold()))
        );
    }

    #[rstest]
    fn emphasis(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str("*Emphasis*"),
            Text::from(Line::from("Emphasis".italic()))
        );
    }

    #[rstest]
    fn strikethrough(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str("~~Strikethrough~~"),
            Text::from(Line::from("Strikethrough".crossed_out()))
        );
    }

    #[rstest]
    fn strong_emphasis(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str("**Strong *emphasis***"),
            Text::from(Line::from_iter([
                "Strong ".bold(),
                "emphasis".bold().italic()
            ]))
        );
    }

    #[rstest]
    fn link(_with_tracing: DefaultGuard) {
        let link_style = Style::new().blue().underlined();
        assert_eq!(
            from_str("[Link](https://example.com)"),
            Text::from(Line::from_iter([
                Span::styled("Link", link_style),
                Span::from(" ("),
                Span::styled("https://example.com", link_style),
                Span::from(")")
            ]))
        );
    }

    mod image {
        use pretty_assertions::assert_eq;

        use super::*;

        const IMAGE_STYLE: Style = Style::new().dim().italic();

        #[rstest]
        fn image_with_alt(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("![Alt text](https://example.com/image.png)"),
                Text::from(Line::from(Span::styled("[img] Alt text", IMAGE_STYLE,)))
            );
        }

        #[rstest]
        fn image_without_alt(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("![](https://example.com/image.png)"),
                Text::from(Line::from(Span::styled(
                    "[img] https://example.com/image.png",
                    IMAGE_STYLE,
                )))
            );
        }

        #[rstest]
        fn image_with_title(_with_tracing: DefaultGuard) {
            // Title is in the markdown syntax but the alt text is what we render.
            assert_eq!(
                from_str("![Alt](https://example.com/img.png \"My Title\")"),
                Text::from(Line::from(Span::styled("[img] Alt", IMAGE_STYLE)))
            );
        }

        #[rstest]
        fn image_in_paragraph(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("Before ![photo](url.png) after"),
                Text::from(Line::from_iter([
                    Span::from("Before "),
                    Span::styled("[img] photo", IMAGE_STYLE),
                    Span::from(" after"),
                ]))
            );
        }
    }

    mod html {
        use pretty_assertions::assert_eq;

        use super::*;

        #[rstest]
        fn inline_html_tag(_with_tracing: DefaultGuard) {
            let text = from_str("Hello <em>world</em>");
            // Inline HTML tags are rendered as dim text alongside normal text.
            assert_eq!(text.lines.len(), 1);
            assert!(text.lines[0].spans.len() >= 2);
        }

        #[rstest]
        fn html_block_renders(_with_tracing: DefaultGuard) {
            let text = from_str("<div>Custom HTML</div>\n");
            // HTML blocks should render something (not be silently dropped).
            assert!(!text.lines.is_empty());
        }
    }

    mod math {
        use pretty_assertions::assert_eq;
        use ratatui::style::Color;

        use super::*;

        #[rstest]
        fn inline_math_renders(_with_tracing: DefaultGuard) {
            let text = from_str("The formula $E=mc^2$ is famous.");
            let line = &text.lines[0];
            // Should contain the math text styled with magenta italic.
            let math_span = line
                .spans
                .iter()
                .find(|s| s.content.contains("E=mc^2"))
                .expect("math span should exist");
            assert_eq!(math_span.style, Style::new().italic().fg(Color::Magenta));
        }

        #[rstest]
        fn display_math_renders(_with_tracing: DefaultGuard) {
            let text = from_str("$$\nx^2 + y^2 = z^2\n$$");
            // Display math should produce output with magenta styling.
            let has_math = text
                .lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("x^2")));
            assert!(has_math, "display math should render the formula");
        }

        #[rstest]
        fn inline_math_has_dim_delimiters(_with_tracing: DefaultGuard) {
            let text = from_str("$\\alpha$");
            let line = &text.lines[0];
            // Delimiters should be separate dim spans.
            let dollar_spans: Vec<_> = line
                .spans
                .iter()
                .filter(|s| s.content.as_ref() == "$")
                .collect();
            assert_eq!(dollar_spans.len(), 2, "should have two $ delimiter spans");
            assert_eq!(dollar_spans[0].style, Style::new().dark_gray());
            // Content span should be magenta italic.
            let content_span = line
                .spans
                .iter()
                .find(|s| s.content.contains("\\alpha"))
                .expect("math content span");
            assert_eq!(content_span.style, Style::new().italic().fg(Color::Magenta));
        }
    }

    mod footnotes {
        use super::*;

        #[rstest]
        fn footnote_reference_renders(_with_tracing: DefaultGuard) {
            let text = from_str("Text[^1]\n\n[^1]: The footnote content.");
            // Should have the reference rendered as [1].
            let has_ref = text
                .lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("[1]")));
            assert!(has_ref, "footnote reference should render as [1]");
        }

        #[rstest]
        fn footnote_definition_renders(_with_tracing: DefaultGuard) {
            let text = from_str("Text[^note]\n\n[^note]: A longer note.");
            let has_def = text
                .lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("[note]:")));
            assert!(has_def, "footnote definition should render");
        }
    }

    mod definition_list {
        use super::*;

        #[rstest]
        fn basic_definition_list(_with_tracing: DefaultGuard) {
            let text = from_str(indoc! {"
                Term 1
                : Definition 1
            "});
            // Term should be bold.
            let has_term = text
                .lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("Term 1")));
            assert!(has_term, "definition list should render the term");
            // Definition should be indented.
            let has_def = text
                .lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("Definition 1")));
            assert!(has_def, "definition list should render the definition");
        }

        #[rstest]
        fn definition_list_indentation(_with_tracing: DefaultGuard) {
            let text = from_str(indoc! {"
                Term
                : Description here
            "});
            // The definition line should start with a colon prefix.
            let def_line = text
                .lines
                .iter()
                .find(|l| l.spans.iter().any(|s| s.content.contains("Description")))
                .expect("should have definition line");
            let has_colon_prefix = def_line.spans.iter().any(|s| s.content.contains(": "));
            assert!(has_colon_prefix, "definition should have colon prefix");
        }
    }

    mod gfm_alerts {
        use super::*;

        #[rstest]
        fn note_alert(_with_tracing: DefaultGuard) {
            let text = from_str("> [!NOTE]\n> This is a note.");
            // Should contain the note icon and label.
            let has_note = text
                .lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("Note")));
            assert!(has_note, "note alert should render Note label");
        }

        #[rstest]
        fn warning_alert(_with_tracing: DefaultGuard) {
            let text = from_str("> [!WARNING]\n> Be careful!");
            let has_warning = text
                .lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("Warning")));
            assert!(has_warning, "warning alert should render Warning label");
        }

        #[rstest]
        fn tip_alert(_with_tracing: DefaultGuard) {
            let text = from_str("> [!TIP]\n> A helpful tip.");
            let has_tip = text
                .lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("Tip")));
            assert!(has_tip, "tip alert should render Tip label");
        }
    }

    mod link_styling {
        use pretty_assertions::assert_eq;

        use super::*;

        #[rstest]
        fn link_text_is_styled(_with_tracing: DefaultGuard) {
            let link_style = Style::new().blue().underlined();
            let text = from_str("[Click here](https://example.com)");
            let line = &text.lines[0];
            // The link text "Click here" should have the link style.
            let text_span = line
                .spans
                .iter()
                .find(|s| s.content.contains("Click here"))
                .expect("link text span");
            assert_eq!(text_span.style, link_style);
        }

        #[rstest]
        fn bold_inside_link(_with_tracing: DefaultGuard) {
            let text = from_str("[**Bold link**](https://example.com)");
            let line = &text.lines[0];
            // The bold link text should combine bold + link styles.
            let bold_span = line
                .spans
                .iter()
                .find(|s| s.content.contains("Bold link"))
                .expect("bold link span");
            assert!(bold_span
                .style
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD));
            assert!(bold_span
                .style
                .add_modifier
                .contains(ratatui::style::Modifier::UNDERLINED));
        }
    }

    mod code_theme {
        use super::*;

        #[rstest]
        fn invalid_theme_does_not_panic(_with_tracing: DefaultGuard) {
            let options = Options::default().code_theme("nonexistent-theme");
            // Should fall back to the default theme, not panic.
            let _text = from_str_with_options("```rust\nfn main() {}\n```", &options);
        }

        #[rstest]
        #[cfg(feature = "highlight-code")]
        fn different_theme_produces_different_output(_with_tracing: DefaultGuard) {
            let default_out = from_str("```rust\nfn main() {}\n```");
            let options = Options::default().code_theme("InspiredGitHub");
            let custom_out = from_str_with_options("```rust\nfn main() {}\n```", &options);
            // The two themes should produce different styled output.
            assert_ne!(
                format!("{default_out:?}"),
                format!("{custom_out:?}"),
                "Different themes should produce different styled output"
            );
        }

        #[rstest]
        #[cfg(feature = "highlight-code")]
        fn available_themes_not_empty(_with_tracing: DefaultGuard) {
            let themes = crate::available_themes();
            assert!(!themes.is_empty());
            assert!(themes.contains(&"base16-ocean.dark"));
        }
    }

    include!("table_tests.rs");
}
