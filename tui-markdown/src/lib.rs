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
use itertools::Itertools;
use pulldown_cmark::{
    BlockQuoteKind, CodeBlockKind, CowStr, Event, HeadingLevel, Options as ParseOptions, Parser,
    Tag, TagEnd,
};
use ratatui_core::style::{Style, Stylize};
use ratatui_core::text::{Line, Span, Text};
#[cfg(feature = "highlight-code")]
use syntect::{
    easy::HighlightLines,
    highlighting::Theme,
    parsing::SyntaxSet,
    util::{as_24_bit_terminal_escaped, LinesWithEndings},
};
use tracing::{debug, instrument, warn};

#[doc(inline)]
#[cfg(feature = "highlight-code")]
pub use crate::code_theme::{BuiltinCodeTheme, CodeTheme};
pub use crate::options::{ImageFallback, Options};
pub use crate::style_sheet::{AlertKind, DefaultStyleSheet, StyleSheet};

#[cfg(feature = "highlight-code")]
mod code_theme;
mod options;
mod style_sheet;
mod tables;

const IMAGE_INDICATOR: &str = "[img]";

/// Render Markdown `input` into a [`Text`] using the default [`Options`].
///
/// This is a convenience function that uses the default options, which are defined in
/// [`Options::default`]. Image syntax renders as a styled text fallback so its description or
/// destination remains visible in terminals without image graphics.
pub fn from_str(input: &str) -> Text<'_> {
    from_str_with_options(input, &Options::default())
}

/// Render Markdown `input` into a [`Text`] using the supplied [`Options`].
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
    parse_opts.insert(ParseOptions::ENABLE_MATH);
    parse_opts.insert(ParseOptions::ENABLE_FOOTNOTES);
    parse_opts.insert(ParseOptions::ENABLE_DEFINITION_LIST);
    parse_opts.insert(ParseOptions::ENABLE_GFM);
    parse_opts.insert(ParseOptions::ENABLE_TABLES);
    let parser = Parser::new_ext(input, parse_opts);

    let mut writer = TextWriter::new(parser, options.styles.clone(), options.image_fallback);
    #[cfg(feature = "highlight-code")]
    writer.set_code_theme(options.selected_code_theme());
    writer.run();
    writer.text
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

fn alert_kind(kind: BlockQuoteKind) -> AlertKind {
    match kind {
        BlockQuoteKind::Note => AlertKind::Note,
        BlockQuoteKind::Tip => AlertKind::Tip,
        BlockQuoteKind::Important => AlertKind::Important,
        BlockQuoteKind::Warning => AlertKind::Warning,
        BlockQuoteKind::Caution => AlertKind::Caution,
    }
}

/// Records how an active list item occupies the rendered output.
///
/// List markers are written as soon as pulldown-cmark emits an item start, but tables are buffered
/// until the matching table end. Remembering the marker's location lets the completed table attach
/// to that already-written marker instead of appearing as a separate, unindented block. These
/// values form a stack because an outer item remains active while a nested item is parsed.
#[derive(Clone, Copy, Debug)]
struct ListItemLayout {
    /// Output line containing the list marker.
    marker_line: usize,
    /// Number of spans on `marker_line` immediately after writing the marker.
    ///
    /// Comparing this with the eventual span count distinguishes a table that is the item's first
    /// content from a table following text on the same line.
    marker_span_count: usize,
    /// Display width reserved by the complete marker, including nesting indentation.
    ///
    /// This uses terminal display width rather than bytes so continuation lines align after
    /// unordered markers and multi-digit ordered markers alike.
    continuation_width: usize,
}

/// An image whose description is still being emitted by pulldown-cmark.
///
/// Image descriptions arrive as the same inline event stream used for ordinary text: formatting,
/// code, HTML, math, and even nested images are separate events between the image's start and end.
/// Buffer the rendered spans until the end event so the fallback marker stays at the correct image
/// boundary and the destination is used only when the description produced no content. A stack is
/// required because pulldown-cmark can emit nested image events inside a description.
#[derive(Debug)]
struct PendingImage<'a> {
    destination: CowStr<'a>,
    style: Style,
    description: Vec<Span<'a>>,
}

impl<'a> PendingImage<'a> {
    fn new(destination: CowStr<'a>, style: Style) -> Self {
        Self {
            destination,
            style,
            description: Vec::new(),
        }
    }

    fn push_span(&mut self, span: Span<'a>) {
        self.description.push(span);
    }

    fn into_fallback(self, fallback: ImageFallback) -> Vec<Span<'a>> {
        let Self {
            destination,
            style,
            description,
        } = self;
        let mut content = match fallback {
            ImageFallback::AltText if description.is_empty() => {
                Self::destination_span(destination, style)
            }
            ImageFallback::AltText => description,
            ImageFallback::Url => Self::destination_span(destination, style),
            ImageFallback::AltTextAndUrl if description.is_empty() => {
                Self::destination_span(destination, style)
            }
            ImageFallback::AltTextAndUrl if destination.is_empty() => description,
            ImageFallback::AltTextAndUrl => {
                let mut description = description;
                let destination = format!(" ({destination})");
                description.push(Span::styled(destination, style));
                description
            }
        };

        let indicator = if content.is_empty() {
            IMAGE_INDICATOR.to_owned()
        } else {
            format!("{IMAGE_INDICATOR} ")
        };
        content.insert(0, Span::styled(indicator, style));
        content
    }

    fn destination_span(destination: CowStr<'a>, style: Style) -> Vec<Span<'a>> {
        if destination.is_empty() {
            Vec::new()
        } else {
            vec![Span::styled(destination, style)]
        }
    }
}

struct TextWriter<'a, 'theme, I, S: StyleSheet> {
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
    code_highlighter: Option<HighlightLines<'theme>>,

    /// Current list index as a stack of indices.
    list_indices: Vec<Option<u64>>,

    /// Layout of each active list item, from the outermost item to the innermost.
    list_items: Vec<ListItemLayout>,

    /// A link which will be appended to the current line when the link tag is closed.
    link: Option<CowStr<'a>>,

    /// Images whose descriptions are currently being collected.
    images: Vec<PendingImage<'a>>,

    /// Content to render in place of images.
    image_fallback: ImageFallback,

    /// The [`StyleSheet`] to use to style the output.
    styles: S,

    /// Heading attributes to append after heading content.
    heading_meta: Option<HeadingMeta<'a>>,

    /// Whether we are inside a metadata block.
    in_metadata_block: bool,

    /// Whether we are inside a footnote definition.
    in_footnote_definition: bool,

    /// Whether we are inside a definition-list description.
    in_definition_description: bool,

    /// Active table builder that accumulates cells during table parsing.
    table_builder: Option<tables::TableBuilder<'a>>,

    /// Resolved syntect theme used for code highlighting.
    #[cfg(feature = "highlight-code")]
    code_theme: &'theme Theme,

    /// Keeps the writer's shape consistent when syntax highlighting is disabled.
    #[cfg(not(feature = "highlight-code"))]
    code_theme_lifetime: std::marker::PhantomData<&'theme ()>,

    needs_newline: bool,
}

#[cfg(feature = "highlight-code")]
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

impl<'a, 'theme, I, S> TextWriter<'a, 'theme, I, S>
where
    I: Iterator<Item = Event<'a>>,
    S: StyleSheet,
{
    fn new(iter: I, styles: S, image_fallback: ImageFallback) -> Self {
        Self {
            iter,
            text: Text::default(),
            inline_styles: vec![],
            line_styles: vec![],
            line_prefixes: vec![],
            list_indices: vec![],
            list_items: vec![],
            needs_newline: false,
            #[cfg(feature = "highlight-code")]
            code_highlighter: None,
            link: None,
            images: vec![],
            image_fallback,
            styles,
            heading_meta: None,
            in_metadata_block: false,
            in_footnote_definition: false,
            in_definition_description: false,
            table_builder: None,
            #[cfg(feature = "highlight-code")]
            code_theme: code_theme::default_backend_theme(),
            #[cfg(not(feature = "highlight-code"))]
            code_theme_lifetime: std::marker::PhantomData,
        }
    }

    /// Selects a configured theme before the event loop starts.
    #[cfg(feature = "highlight-code")]
    fn set_code_theme(&mut self, theme: &'theme CodeTheme) {
        self.code_theme = code_theme::backend_theme(theme);
    }

    fn run(&mut self) {
        debug!("Running text writer");
        while let Some(event) = self.iter.next() {
            self.handle_event(event);
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
            Tag::TableHead => {}
            Tag::TableRow => {}
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
            Tag::DefinitionListDefinition => self.start_definition_description(),
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
            TagEnd::Item => self.end_item(),
            TagEnd::FootnoteDefinition => self.end_footnote_definition(),
            TagEnd::Table => self.end_table(),
            TagEnd::TableHead => self.end_table_header(),
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
            TagEnd::DefinitionListDefinition => self.end_definition_description(),
        }
    }

    fn start_paragraph(&mut self) {
        // Footnote definitions and loose definition-list descriptions start with a paragraph event
        // after their handlers have already written a visible prefix (`[label]: ` or `: `) to the
        // current line. Skip normal paragraph handling only for that first paragraph so its content
        // stays beside the prefix. For a later paragraph, `needs_newline` is true; allowing the
        // normal path below to run preserves the blank line in definitions such as:
        //
        //     [^label]: First paragraph.
        //
        //         Second paragraph.
        let prefix_line_is_open = self.in_footnote_definition || self.in_definition_description;
        if prefix_line_is_open && !self.needs_newline {
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

        match kind {
            Some(kind) => self.start_alert(alert_kind(kind)),
            None => self.start_plain_blockquote(),
        }
    }

    fn start_alert(&mut self, kind: AlertKind) {
        let style = self.styles.alert(kind);
        self.push_blockquote_style(style);
        self.push_line(Line::default());
        self.push_span(Span::styled(self.alert_heading(kind), style.bold()));
        self.needs_newline = false;
    }

    fn start_plain_blockquote(&mut self) {
        self.push_blockquote_style(self.styles.blockquote());
    }

    fn push_blockquote_style(&mut self, style: Style) {
        self.line_prefixes.push(Span::from(">"));
        self.line_styles.push(style);
    }

    fn alert_heading(&self, kind: AlertKind) -> String {
        let icon = self.styles.alert_icon(kind);
        let label = self.styles.alert_label(kind);
        // Either component may be intentionally suppressed; add a separator only when both exist.
        match (icon.is_empty(), label.is_empty()) {
            (false, false) => format!("{icon} {label}"),
            (false, true) => icon.to_owned(),
            (true, false) => label.to_owned(),
            (true, true) => String::new(),
        }
    }

    fn end_blockquote(&mut self) {
        self.line_prefixes.pop();
        self.line_styles.pop();
        self.needs_newline = true;
    }

    fn text(&mut self, text: CowStr<'a>) {
        if self.table_builder.is_some() {
            let style = self.inline_styles.last().copied().unwrap_or_default();
            self.push_span(Span::styled(text, style));
            return;
        }

        #[cfg(feature = "highlight-code")]
        if let Some(highlighter) = &mut self.code_highlighter {
            let text: Text = LinesWithEndings::from(&text)
                .filter_map(|line| highlighter.highlight_line(line, &SYNTAX_SET).ok())
                .filter_map(|part| as_24_bit_terminal_escaped(&part, false).into_text().ok())
                .flatten()
                .collect();

            for line in text.lines {
                self.text.push_line(line);
            }
            self.needs_newline = false;
            return;
        }

        for (position, line) in text.lines().with_position() {
            if self.needs_newline {
                self.push_line(Line::default());
                self.needs_newline = false;
            }
            if !position.is_first() {
                self.push_line(Line::default());
            }

            let style = self.inline_styles.last().copied().unwrap_or_default();

            let span = Span::styled(line.to_owned(), style);

            self.push_span(span);
        }
        self.needs_newline = false;
    }

    fn code(&mut self, code: CowStr<'a>) {
        let style = if self.images.is_empty() {
            self.styles.code()
        } else {
            let inline_style = self.inline_styles.last().copied().unwrap_or_default();
            inline_style.patch(self.styles.code())
        };

        let span = Span::styled(code, style);
        self.push_span(span);
    }

    fn hard_break(&mut self) {
        if self.images.is_empty() {
            self.push_line(Line::default());
        } else {
            self.image_description_break();
        }
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
        self.push_line(Line::from("---"));
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
        let marker_line = self.text.lines.len();
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
            let continuation_width = span.width();
            self.push_span(span);
            let marker_span_count = self.text.lines[marker_line].spans.len();
            self.list_items.push(ListItemLayout {
                marker_line,
                marker_span_count,
                continuation_width,
            });
        }
        self.needs_newline = false;
    }

    fn end_item(&mut self) {
        self.list_items.pop();
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
        } else if self.images.is_empty() {
            self.push_span(Span::raw(" "));
        } else {
            self.image_description_break();
        }
    }

    fn image_description_break(&mut self) {
        // Image descriptions are inline content. Keep a break readable without allowing it to
        // split the surrounding document, and retain the image style in case it has a background.
        let style = self.inline_styles.last().copied().unwrap_or_default();
        self.push_span(Span::styled(" ", style));
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

        let span = Span::from(format!("```{lang}"));
        self.push_line(span.into());
        self.needs_newline = true;
    }

    fn end_codeblock(&mut self) {
        let span = Span::from("```");
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
            let highlighter = HighlightLines::new(syntax, self.code_theme);
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
        // An active image owns every span produced by its inline event stream. Checking it before
        // the table sink also lets a completed fallback enter a table cell as one ordered unit.
        if let Some(image) = self.images.last_mut() {
            image.push_span(span);
            return;
        }

        // GFM tables are leaf blocks: their cells parse inline content, and block-level elements
        // cannot occur inside them. Pulldown-cmark preserves that boundary by emitting only inline
        // events inside `TableCell`. Keep the active cell as the single span sink anyway so a new
        // inline event handler cannot accidentally write table content into the surrounding text.
        // See <https://github.github.com/gfm/#tables-extension->.
        if let Some(builder) = &mut self.table_builder {
            builder.push_span(span);
            return;
        }

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

    /// Begin collecting the rendered image description.
    #[instrument(level = "trace", skip(self))]
    fn start_image(&mut self, dest_url: CowStr<'a>) {
        self.push_inline_style(self.styles.image_alt());
        let style = self.inline_styles.last().copied().unwrap_or_default();
        self.images.push(PendingImage::new(dest_url, style));
    }

    /// Finish the current image and emit its text fallback to the enclosing output.
    ///
    /// Pop the image before emitting so a nested image becomes part of its parent's description,
    /// while an outer image continues through the usual table or document span sink.
    #[instrument(level = "trace", skip(self))]
    fn end_image(&mut self) {
        self.pop_inline_style();
        if let Some(image) = self.images.pop() {
            for span in image.into_fallback(self.image_fallback) {
                self.push_span(span);
            }
        }
    }

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
        let inline_style = self.inline_styles.last().copied().unwrap_or_default();
        let style = inline_style.patch(self.styles.html());
        self.push_span(Span::styled(html, style));
    }

    fn inline_math(&mut self, math: CowStr<'a>) {
        let inline_style = self.inline_styles.last().copied().unwrap_or_default();
        let style = inline_style.patch(self.styles.math_inline());
        self.push_span(Span::styled(format!("${math}$"), style));
    }

    fn display_math(&mut self, math: CowStr<'a>) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        let style = self.styles.math_display();
        let display_math = format!("$${math}$$");
        for (index, line) in display_math.lines().enumerate() {
            if index > 0 {
                self.push_line(Line::default());
            }
            self.push_span(Span::styled(line.to_owned(), style));
        }
        self.needs_newline = true;
    }

    fn footnote_reference(&mut self, label: CowStr<'a>) {
        // A reference can appear inside other inline formatting, as in `**Text[^label]**`.
        // Styling it with only `footnote_ref()` would make `[label]` dim and italic but drop the
        // surrounding bold style. Start with the active inline style and patch the footnote style
        // over it so the reference adds its own appearance without losing enclosing formatting.
        let inline_style = self.inline_styles.last().copied().unwrap_or_default();
        let style = inline_style.patch(self.styles.footnote_ref());
        self.push_span(Span::styled(format!("[{label}]"), style));
    }

    fn start_footnote_definition(&mut self, label: CowStr<'a>) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        let style = self.styles.footnote_def();
        self.line_styles.push(style);
        self.push_line(Line::default());
        self.push_span(Span::styled(format!("[{label}]: "), style));
        self.in_footnote_definition = true;
        self.needs_newline = false;
    }

    fn end_footnote_definition(&mut self) {
        self.line_styles.pop();
        self.in_footnote_definition = false;
        self.needs_newline = true;
    }

    fn start_definition_list(&mut self) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        self.needs_newline = false;
    }

    fn end_definition_list(&mut self) {
        self.needs_newline = true;
    }

    fn start_definition_title(&mut self) {
        // Definition-list terms contain inline events without an ordinary paragraph start, so the
        // term handler owns the output line and applies the term style before consuming them.
        self.push_line(Line::default());
        self.push_inline_style(self.styles.definition_term());
        self.needs_newline = false;
    }

    fn end_definition_title(&mut self) {
        self.pop_inline_style();
        self.needs_newline = false;
    }

    fn start_definition_description(&mut self) {
        // A tight description contains inline events without an ordinary paragraph start. Create
        // its line here and write the visible Markdown `: ` marker before consuming the content so
        // the marker and content share the description style.
        self.push_line(Line::default());
        self.push_span(Span::styled(": ", self.styles.definition_description()));
        self.push_inline_style(self.styles.definition_description());
        self.in_definition_description = true;
        self.needs_newline = false;
    }

    fn end_definition_description(&mut self) {
        self.pop_inline_style();
        self.in_definition_description = false;
        self.needs_newline = false;
    }

    fn start_table(&mut self, alignments: Vec<pulldown_cmark::Alignment>) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        self.table_builder = Some(tables::TableBuilder::new(alignments));
        self.needs_newline = false;
    }

    fn end_table_header(&mut self) {
        if let Some(builder) = &mut self.table_builder {
            builder.finish_header();
        }
    }

    fn end_table_row(&mut self) {
        if let Some(builder) = &mut self.table_builder {
            builder.finish_row();
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

    fn end_table(&mut self) {
        if let Some(builder) = self.table_builder.take() {
            let lines = builder.render(&self.styles);
            self.push_table_lines(lines);
            self.needs_newline = true;
        }
    }

    /// Adds a buffered table to the output while preserving an active list item's layout.
    ///
    /// A table that is the first content in an item starts on the marker line. Its remaining lines
    /// are indented by the marker's display width. A later table cannot reuse the marker line, but
    /// all of its lines still need the continuation indentation.
    ///
    /// Table rendering currently puts styles on individual spans and leaves the line style and
    /// alignment at their defaults. This makes it safe to move the first rendered line's spans
    /// onto the existing marker line.
    fn push_table_lines(&mut self, lines: Vec<Line<'a>>) {
        let Some(list_item) = self.list_items.last().copied() else {
            for line in lines {
                self.push_line(line);
            }
            return;
        };

        let mut lines = lines.into_iter();
        // The line position alone is insufficient: inline item content may already have appended
        // spans to the marker line before the table was buffered.
        let marker_line_is_last = self.text.lines.len() == list_item.marker_line + 1;
        let marker_has_no_content =
            self.text.lines[list_item.marker_line].spans.len() == list_item.marker_span_count;
        let table_starts_on_marker = marker_line_is_last && marker_has_no_content;
        if table_starts_on_marker {
            if let Some(first_line) = lines.next() {
                self.text.lines[list_item.marker_line]
                    .spans
                    .extend(first_line.spans);
            }
        }

        let continuation = " ".repeat(list_item.continuation_width);
        for mut line in lines {
            line.spans.insert(0, Span::raw(continuation.clone()));
            self.push_line(line);
        }
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

    mod footnotes {
        use pretty_assertions::assert_eq;

        use super::*;

        #[rstest]
        fn multiline_definition_has_exact_layout(_with_tracing: DefaultGuard) {
            let reference_style = Style::new().dim().italic();
            let definition_style = Style::new().dim();

            assert_eq!(
                from_str("Text[^one]\n\n[^one]: First line\n    continued line."),
                Text::from_iter([
                    Line::from_iter([Span::raw("Text"), Span::styled("[one]", reference_style),]),
                    Line::default(),
                    Line::from_iter([
                        Span::styled("[one]: ", definition_style),
                        Span::raw("First line"),
                        Span::raw(" "),
                        Span::raw("continued line."),
                    ])
                    .style(definition_style),
                ])
            );
        }

        #[rstest]
        fn multiple_definitions_have_exact_layout(_with_tracing: DefaultGuard) {
            let reference_style = Style::new().dim().italic();
            let definition_style = Style::new().dim();

            assert_eq!(
                from_str("First[^a] second[^b].\n\n[^a]: Alpha.\n\n[^b]: Beta."),
                Text::from_iter([
                    Line::from_iter([
                        Span::raw("First"),
                        Span::styled("[a]", reference_style),
                        Span::raw(" second"),
                        Span::styled("[b]", reference_style),
                        Span::raw("."),
                    ]),
                    Line::default(),
                    Line::from_iter(
                        [Span::styled("[a]: ", definition_style), Span::raw("Alpha."),]
                    )
                    .style(definition_style),
                    Line::default(),
                    Line::from_iter([Span::styled("[b]: ", definition_style), Span::raw("Beta."),])
                        .style(definition_style),
                ])
            );
        }

        #[rstest]
        fn reference_combines_with_enclosing_style(_with_tracing: DefaultGuard) {
            let reference_style = Style::new().bold().dim().italic();
            assert_eq!(
                from_str("**Text[^one]**\n\n[^one]: Note."),
                Text::from_iter([
                    Line::from_iter([
                        Span::styled("Text", Style::new().bold()),
                        Span::styled("[one]", reference_style),
                    ]),
                    Line::default(),
                    Line::from_iter([
                        Span::styled("[one]: ", Style::new().dim()),
                        Span::raw("Note."),
                    ])
                    .style(Style::new().dim()),
                ])
            );
        }

        #[rstest]
        fn multiple_definition_paragraphs_keep_blank_line(_with_tracing: DefaultGuard) {
            let definition_style = Style::new().dim();
            assert_eq!(
                from_str("Text[^one]\n\n[^one]: First paragraph.\n\n    Second paragraph."),
                Text::from_iter([
                    Line::from_iter([
                        Span::raw("Text"),
                        Span::styled("[one]", definition_style.italic()),
                    ]),
                    Line::default(),
                    Line::from_iter([
                        Span::styled("[one]: ", definition_style),
                        Span::raw("First paragraph."),
                    ])
                    .style(definition_style),
                    Line::default().style(definition_style),
                    Line::from("Second paragraph.").style(definition_style),
                ])
            );
        }

        #[rstest]
        fn definition_style_does_not_leak_into_following_paragraph(_with_tracing: DefaultGuard) {
            let definition_style = Style::new().dim();
            assert_eq!(
                from_str(
                    "Text[^one]\n\n[^one]: First paragraph.\n\n    Second paragraph.\n\nAfter."
                ),
                Text::from_iter([
                    Line::from_iter([
                        Span::raw("Text"),
                        Span::styled("[one]", definition_style.italic()),
                    ]),
                    Line::default(),
                    Line::from_iter([
                        Span::styled("[one]: ", definition_style),
                        Span::raw("First paragraph."),
                    ])
                    .style(definition_style),
                    Line::default().style(definition_style),
                    Line::from("Second paragraph.").style(definition_style),
                    Line::default(),
                    Line::from("After."),
                ])
            );
        }

        #[rstest]
        fn custom_styles_compose_with_enclosing_formatting(_with_tracing: DefaultGuard) {
            #[derive(Clone, Copy)]
            struct CustomFootnoteStyle;

            impl StyleSheet for CustomFootnoteStyle {
                fn heading(&self, level: u8) -> Style {
                    DefaultStyleSheet.heading(level)
                }

                fn code(&self) -> Style {
                    DefaultStyleSheet.code()
                }

                fn link(&self) -> Style {
                    DefaultStyleSheet.link()
                }

                fn blockquote(&self) -> Style {
                    DefaultStyleSheet.blockquote()
                }

                fn heading_meta(&self) -> Style {
                    DefaultStyleSheet.heading_meta()
                }

                fn metadata_block(&self) -> Style {
                    DefaultStyleSheet.metadata_block()
                }

                fn footnote_ref(&self) -> Style {
                    Style::new().red().underlined()
                }

                fn footnote_def(&self) -> Style {
                    Style::new().blue().underlined()
                }
            }

            let reference_style = Style::new().red().bold().underlined();
            let definition_style = Style::new().blue().underlined();
            let options = Options::new(CustomFootnoteStyle);
            assert_eq!(
                from_str_with_options("**Text[^one]**\n\n[^one]: Note.", &options),
                Text::from_iter([
                    Line::from_iter([
                        Span::styled("Text", Style::new().bold()),
                        Span::styled("[one]", reference_style),
                    ]),
                    Line::default(),
                    Line::from_iter([
                        Span::styled("[one]: ", definition_style),
                        Span::raw("Note."),
                    ])
                    .style(definition_style),
                ])
            );
        }
    }

    mod definition_list {
        use pretty_assertions::assert_eq;

        use super::*;

        #[derive(Clone)]
        struct CustomDefinitionStyleSheet;

        impl StyleSheet for CustomDefinitionStyleSheet {
            fn heading(&self, level: u8) -> Style {
                DefaultStyleSheet.heading(level)
            }

            fn code(&self) -> Style {
                DefaultStyleSheet.code()
            }

            fn link(&self) -> Style {
                DefaultStyleSheet.link()
            }

            fn blockquote(&self) -> Style {
                DefaultStyleSheet.blockquote()
            }

            fn heading_meta(&self) -> Style {
                DefaultStyleSheet.heading_meta()
            }

            fn metadata_block(&self) -> Style {
                DefaultStyleSheet.metadata_block()
            }

            fn definition_term(&self) -> Style {
                Style::new().red().underlined()
            }

            fn definition_description(&self) -> Style {
                Style::new().blue().italic()
            }
        }

        #[rstest]
        fn exact_output_and_default_styles(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("Term\n: Definition\n"),
                Text::from_iter([
                    Line::from(Span::styled("Term", Style::new().bold())),
                    Line::from_iter([Span::raw(": "), Span::raw("Definition")]),
                ])
            );
        }

        #[rstest]
        fn custom_styles_apply_to_terms_and_definitions(_with_tracing: DefaultGuard) {
            let options = Options::new(CustomDefinitionStyleSheet);
            let text = from_str_with_options("Term\n: Definition\n", &options);
            let title_style = Style::new().red().underlined();
            let description_style = Style::new().blue().italic();

            assert_eq!(
                text,
                Text::from_iter([
                    Line::from(Span::styled("Term", title_style)),
                    Line::from_iter([
                        Span::styled(": ", description_style),
                        Span::styled("Definition", description_style),
                    ]),
                ])
            );
        }

        #[rstest]
        fn inline_formatting_combines_with_definition_styles(_with_tracing: DefaultGuard) {
            let options = Options::new(CustomDefinitionStyleSheet);
            let term_style = Style::new().red().underlined().italic();
            let description_style = Style::new().blue().italic();

            assert_eq!(
                from_str_with_options("*Term*\n: **Description**\n", &options),
                Text::from_iter([
                    Line::from(Span::styled("Term", term_style)),
                    Line::from_iter([
                        Span::styled(": ", description_style),
                        Span::styled("Description", description_style.bold()),
                    ]),
                ])
            );
        }

        #[rstest]
        fn multiline_definition(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("Term\n: First line\n  second line\n"),
                Text::from_iter([
                    Line::from(Span::styled("Term", Style::new().bold())),
                    Line::from_iter([
                        Span::raw(": "),
                        Span::raw("First line"),
                        Span::raw(" "),
                        Span::raw("second line"),
                    ]),
                ])
            );
        }

        #[rstest]
        fn multiple_descriptions(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("Term\n: First description\n: Second description\n"),
                Text::from_iter([
                    Line::from(Span::styled("Term", Style::new().bold())),
                    Line::from_iter([Span::raw(": "), Span::raw("First description")]),
                    Line::from_iter([Span::raw(": "), Span::raw("Second description")]),
                ])
            );
        }

        #[rstest]
        fn multiple_description_paragraphs_keep_prefix_and_blank_line(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("Term\n: First paragraph.\n\n  Second paragraph."),
                Text::from_iter([
                    Line::from(Span::styled("Term", Style::new().bold())),
                    Line::from_iter([Span::raw(": "), Span::raw("First paragraph.")]),
                    Line::default(),
                    Line::from("Second paragraph."),
                ])
            );
        }

        #[rstest]
        fn repeated_items_do_not_leak_into_following_paragraph(_with_tracing: DefaultGuard) {
            let term_style = Style::new().bold();
            assert_eq!(
                from_str(
                    "Term one\n: First description.\n\nTerm two\n: Second description.\n\nAfter."
                ),
                Text::from_iter([
                    Line::from(Span::styled("Term one", term_style)),
                    Line::from_iter([Span::raw(": "), Span::raw("First description.")]),
                    Line::from(Span::styled("Term two", term_style)),
                    Line::from_iter([Span::raw(": "), Span::raw("Second description.")]),
                    Line::default(),
                    Line::from("After."),
                ])
            );
        }
    }

    mod gfm_alerts {
        use pretty_assertions::assert_eq;

        use super::*;

        #[derive(Clone)]
        struct CustomAlertStyleSheet;

        impl StyleSheet for CustomAlertStyleSheet {
            fn heading(&self, level: u8) -> Style {
                DefaultStyleSheet.heading(level)
            }

            fn code(&self) -> Style {
                DefaultStyleSheet.code()
            }

            fn link(&self) -> Style {
                DefaultStyleSheet.link()
            }

            fn blockquote(&self) -> Style {
                DefaultStyleSheet.blockquote()
            }

            fn heading_meta(&self) -> Style {
                DefaultStyleSheet.heading_meta()
            }

            fn metadata_block(&self) -> Style {
                DefaultStyleSheet.metadata_block()
            }

            fn alert(&self, kind: AlertKind) -> Style {
                match kind {
                    AlertKind::Note => Style::new().on_red(),
                    _ => Style::default(),
                }
            }
        }

        #[derive(Clone)]
        struct CustomAlertHeadingStyleSheet;

        impl StyleSheet for CustomAlertHeadingStyleSheet {
            fn heading(&self, level: u8) -> Style {
                DefaultStyleSheet.heading(level)
            }

            fn code(&self) -> Style {
                DefaultStyleSheet.code()
            }

            fn link(&self) -> Style {
                DefaultStyleSheet.link()
            }

            fn blockquote(&self) -> Style {
                DefaultStyleSheet.blockquote()
            }

            fn heading_meta(&self) -> Style {
                DefaultStyleSheet.heading_meta()
            }

            fn metadata_block(&self) -> Style {
                DefaultStyleSheet.metadata_block()
            }

            fn alert_icon(&self, kind: AlertKind) -> &str {
                match kind {
                    AlertKind::Note => "!!",
                    AlertKind::Caution => "",
                    _ => DefaultStyleSheet.alert_icon(kind),
                }
            }

            fn alert_label(&self, kind: AlertKind) -> &str {
                match kind {
                    AlertKind::Tip => "Hint",
                    AlertKind::Important => "",
                    _ => kind.label(),
                }
            }
        }

        #[rstest]
        #[case("NOTE", "\u{2139}\u{FE0F} Note", Style::new().blue())]
        #[case("TIP", "\u{1F4A1} Tip", Style::new().green())]
        #[case("IMPORTANT", "\u{2757} Important", Style::new().magenta())]
        #[case("WARNING", "\u{26A0}\u{FE0F} Warning", Style::new().yellow())]
        #[case("CAUTION", "\u{1F534} Caution", Style::new().red())]
        fn alert_kind_renders_exact_output(
            _with_tracing: DefaultGuard,
            #[case] marker: &str,
            #[case] heading: &str,
            #[case] style: Style,
        ) {
            let markdown = format!("> [!{marker}]\n> Body");
            assert_eq!(
                from_str(&markdown),
                Text::from_iter([
                    Line::from_iter([
                        Span::raw(">"),
                        Span::raw(" "),
                        Span::styled(heading.to_owned(), style.bold()),
                    ])
                    .style(style),
                    Line::from_iter([Span::raw(">"), Span::raw(" "), Span::raw("Body")])
                        .style(style),
                ])
            );
        }

        #[rstest]
        fn custom_alert_style_applies_to_header_and_body(_with_tracing: DefaultGuard) {
            let style = Style::new().on_red();
            let options = Options::new(CustomAlertStyleSheet);

            assert_eq!(
                from_str_with_options("> [!NOTE]\n> Body", &options),
                Text::from_iter([
                    Line::from_iter([
                        Span::raw(">"),
                        Span::raw(" "),
                        Span::styled("\u{2139}\u{FE0F} Note", style.bold()),
                    ])
                    .style(style),
                    Line::from_iter([Span::raw(">"), Span::raw(" "), Span::raw("Body")])
                        .style(style),
                ])
            );
        }

        #[rstest]
        fn custom_alert_icon_replaces_default(_with_tracing: DefaultGuard) {
            let style = DefaultStyleSheet.alert(AlertKind::Note);
            let options = Options::new(CustomAlertHeadingStyleSheet);

            assert_eq!(
                from_str_with_options("> [!NOTE]\n> Body", &options),
                Text::from_iter([
                    Line::from_iter([
                        Span::raw(">"),
                        Span::raw(" "),
                        Span::styled("!! Note", style.bold()),
                    ])
                    .style(style),
                    Line::from_iter([Span::raw(">"), Span::raw(" "), Span::raw("Body")])
                        .style(style),
                ])
            );
        }

        #[rstest]
        fn empty_alert_icon_suppresses_icon_and_separator(_with_tracing: DefaultGuard) {
            let style = DefaultStyleSheet.alert(AlertKind::Caution);
            let options = Options::new(CustomAlertHeadingStyleSheet);

            assert_eq!(
                from_str_with_options("> [!CAUTION]\n> Body", &options),
                Text::from_iter([
                    Line::from_iter([
                        Span::raw(">"),
                        Span::raw(" "),
                        Span::styled("Caution", style.bold()),
                    ])
                    .style(style),
                    Line::from_iter([Span::raw(">"), Span::raw(" "), Span::raw("Body")])
                        .style(style),
                ])
            );
        }

        #[rstest]
        fn custom_alert_label_replaces_default(_with_tracing: DefaultGuard) {
            let style = DefaultStyleSheet.alert(AlertKind::Tip);
            let options = Options::new(CustomAlertHeadingStyleSheet);

            assert_eq!(
                from_str_with_options("> [!TIP]\n> Body", &options),
                Text::from_iter([
                    Line::from_iter([
                        Span::raw(">"),
                        Span::raw(" "),
                        Span::styled("💡 Hint", style.bold()),
                    ])
                    .style(style),
                    Line::from_iter([Span::raw(">"), Span::raw(" "), Span::raw("Body")])
                        .style(style),
                ])
            );
        }

        #[rstest]
        fn empty_alert_label_suppresses_label_and_separator(_with_tracing: DefaultGuard) {
            let style = DefaultStyleSheet.alert(AlertKind::Important);
            let options = Options::new(CustomAlertHeadingStyleSheet);

            assert_eq!(
                from_str_with_options("> [!IMPORTANT]\n> Body", &options),
                Text::from_iter([
                    Line::from_iter([
                        Span::raw(">"),
                        Span::raw(" "),
                        Span::styled("❗", style.bold()),
                    ])
                    .style(style),
                    Line::from_iter([Span::raw(">"), Span::raw(" "), Span::raw("Body")])
                        .style(style),
                ])
            );
        }

        #[rstest]
        fn ordinary_blockquote_keeps_standard_prefix(_with_tracing: DefaultGuard) {
            let style = DefaultStyleSheet.blockquote();
            assert_eq!(
                from_str("> Ordinary"),
                Text::from(Line::from_iter([">", " ", "Ordinary"]).style(style))
            );
        }

        #[rstest]
        fn nested_blockquote_keeps_each_standard_prefix(_with_tracing: DefaultGuard) {
            let style = DefaultStyleSheet.blockquote();
            assert_eq!(
                from_str("> Parent\n>> Child"),
                Text::from_iter([
                    Line::from_iter([">", " ", "Parent"]).style(style),
                    Line::from_iter([">", " "]).style(style),
                    Line::from_iter([">", ">", " ", "Child"]).style(style),
                ])
            );
        }

        #[rstest]
        fn alert_preserves_nested_blockquote(_with_tracing: DefaultGuard) {
            let alert_style = Style::new().blue();
            let blockquote_style = DefaultStyleSheet.blockquote();
            assert_eq!(
                from_str("> [!NOTE]\n> Parent\n>> Child"),
                Text::from_iter([
                    Line::from_iter([
                        Span::raw(">"),
                        Span::raw(" "),
                        Span::styled("\u{2139}\u{FE0F} Note", alert_style.bold()),
                    ])
                    .style(alert_style),
                    Line::from_iter([Span::raw(">"), Span::raw(" "), Span::raw("Parent")])
                        .style(alert_style),
                    Line::from_iter([Span::raw(">"), Span::raw(" ")]).style(alert_style),
                    Line::from_iter([
                        Span::raw(">"),
                        Span::raw(">"),
                        Span::raw(" "),
                        Span::raw("Child"),
                    ])
                    .style(blockquote_style),
                ])
            );
        }
    }

    #[rstest]
    fn rule(_with_tracing: DefaultGuard) {
        assert_eq!(
            from_str(indoc! {"
                Paragraph 1

                ---

                Paragraph 2
            "}),
            Text::from_iter(["Paragraph 1", "", "---", "", "Paragraph 2"])
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
                    Line::from_iter([">", " ", "Blockquote"]).style(STYLE),
                ])
            );
        }
        #[rstest]
        fn single(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("> Blockquote"),
                Text::from(Line::from_iter([">", " ", "Blockquote"]).style(STYLE))
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
                    Line::from_iter([">", " ", "Blockquote 1", " ", "Blockquote 2"]).style(STYLE)
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
                    Line::from_iter([">", " ", "Blockquote 1"]).style(STYLE),
                    Line::from_iter([">", " "]).style(STYLE),
                    Line::from_iter([">", " ", "Blockquote 2"]).style(STYLE),
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
                    Line::from_iter([">", " ", "Blockquote 1"]).style(STYLE),
                    Line::default(),
                    Line::from_iter([">", " ", "Blockquote 2"]).style(STYLE),
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
                    Line::from_iter([">", " ", "Blockquote 1"]).style(STYLE),
                    Line::from_iter([">", " "]).style(STYLE),
                    Line::from_iter([">", ">", " ", "Nested Blockquote"]).style(STYLE),
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
    fn link_uses_default_style(_with_tracing: DefaultGuard) {
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
    #[rstest]
    fn link_combines_with_bold_style(_with_tracing: DefaultGuard) {
        let link_style = Style::new().blue().underlined();
        assert_eq!(
            from_str("[**Bold link**](https://example.com)"),
            Text::from(Line::from_iter([
                Span::styled("Bold link", link_style.bold()),
                Span::from(" ("),
                Span::styled("https://example.com", link_style),
                Span::from(")"),
            ]))
        );
    }

    mod html {
        use pretty_assertions::assert_eq;

        use super::*;

        #[rstest]
        fn inline_html_tag(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("Hello <em>world</em>"),
                Text::from(Line::from_iter([
                    Span::from("Hello "),
                    Span::styled("<em>", Style::new().dim()),
                    Span::from("world"),
                    Span::styled("</em>", Style::new().dim()),
                ]))
            );
        }

        #[rstest]
        fn inline_html_combines_with_emphasis(_with_tracing: DefaultGuard) {
            let italic = Style::new().italic();
            let html = italic.dim();
            assert_eq!(
                from_str("*Hello <em>world</em>*"),
                Text::from(Line::from_iter([
                    Span::styled("Hello ", italic),
                    Span::styled("<em>", html),
                    Span::styled("world", italic),
                    Span::styled("</em>", html),
                ]))
            );
        }

        #[rstest]
        fn html_block_preserves_paragraph_spacing(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("Before\n\n<div>\nCustom HTML\n</div>\n\nAfter"),
                Text::from_iter([
                    Line::from("Before"),
                    Line::default(),
                    Line::from(Span::styled("<div>", Style::new().dim())),
                    Line::from(Span::styled("Custom HTML", Style::new().dim()))
                        .style(Style::new().dim()),
                    Line::from(Span::styled("</div>", Style::new().dim()))
                        .style(Style::new().dim()),
                    Line::default(),
                    Line::from("After"),
                ])
            );
        }
    }

    mod math {
        use pretty_assertions::assert_eq;
        use ratatui_core::style::Color;

        use super::*;

        #[rstest]
        fn inline_math_has_exact_output_and_style(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("The formula $E=mc^2$ is famous."),
                Text::from(Line::from_iter([
                    Span::raw("The formula "),
                    Span::styled("$E=mc^2$", Style::new().italic().fg(Color::Magenta)),
                    Span::raw(" is famous."),
                ]))
            );
        }

        #[rstest]
        fn inline_math_combines_with_enclosing_style(_with_tracing: DefaultGuard) {
            let style = Style::new().bold().italic().fg(Color::Magenta);
            assert_eq!(
                from_str("**$x$**"),
                Text::from(Line::from(Span::styled("$x$", style)))
            );
        }

        #[rstest]
        fn multiline_display_math_styles_every_line(_with_tracing: DefaultGuard) {
            let style = Style::new().fg(Color::Magenta);
            assert_eq!(
                from_str("Before\n\n$$\nx = y\ny = z\n$$\n\nAfter"),
                Text::from_iter([
                    Line::from("Before"),
                    Line::default(),
                    Line::from(Span::styled("$$", style)),
                    Line::from(Span::styled("x = y", style)),
                    Line::from(Span::styled("y = z", style)),
                    Line::from(Span::styled("$$", style)),
                    Line::default(),
                    Line::from("After"),
                ])
            );
        }

        #[rstest]
        fn multiline_display_math_uses_custom_style(_with_tracing: DefaultGuard) {
            #[derive(Clone, Copy)]
            struct CustomMathStyle;

            impl StyleSheet for CustomMathStyle {
                fn heading(&self, level: u8) -> Style {
                    DefaultStyleSheet.heading(level)
                }

                fn code(&self) -> Style {
                    DefaultStyleSheet.code()
                }

                fn link(&self) -> Style {
                    DefaultStyleSheet.link()
                }

                fn blockquote(&self) -> Style {
                    DefaultStyleSheet.blockquote()
                }

                fn heading_meta(&self) -> Style {
                    DefaultStyleSheet.heading_meta()
                }

                fn metadata_block(&self) -> Style {
                    DefaultStyleSheet.metadata_block()
                }

                fn math_display(&self) -> Style {
                    Style::new().red().bold()
                }
            }

            let style = Style::new().red().bold();
            let options = Options::new(CustomMathStyle);
            assert_eq!(
                from_str_with_options("$$\nx = y\ny = z\n$$", &options),
                Text::from_iter([
                    Line::from(Span::styled("$$", style)),
                    Line::from(Span::styled("x = y", style)),
                    Line::from(Span::styled("y = z", style)),
                    Line::from(Span::styled("$$", style)),
                ])
            );
        }
    }

    mod image {
        use pretty_assertions::assert_eq;

        use super::*;

        const IMAGE_STYLE: Style = Style::new().dim().italic();

        #[derive(Clone)]
        struct UnstyledImageStyleSheet;

        impl StyleSheet for UnstyledImageStyleSheet {
            fn heading(&self, _level: u8) -> Style {
                Style::default()
            }

            fn code(&self) -> Style {
                Style::default()
            }

            fn link(&self) -> Style {
                Style::default()
            }

            fn blockquote(&self) -> Style {
                Style::default()
            }

            fn heading_meta(&self) -> Style {
                Style::default()
            }

            fn metadata_block(&self) -> Style {
                Style::default()
            }

            fn image_alt(&self) -> Style {
                Style::default()
            }
        }

        #[rstest]
        fn image_with_alt(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("![Alt text](https://example.com/image.png)"),
                Text::from(Line::from_iter([
                    Span::styled("[img] ", IMAGE_STYLE),
                    Span::styled("Alt text", IMAGE_STYLE),
                ]))
            );
        }

        #[rstest]
        fn image_without_alt(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("![](https://example.com/image.png)"),
                Text::from(Line::from_iter([
                    Span::styled("[img] ", IMAGE_STYLE),
                    Span::styled("https://example.com/image.png", IMAGE_STYLE),
                ]))
            );
        }

        #[rstest]
        fn image_with_title(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("![Alt](https://example.com/img.png \"My Title\")"),
                Text::from(Line::from_iter([
                    Span::styled("[img] ", IMAGE_STYLE),
                    Span::styled("Alt", IMAGE_STYLE),
                ]))
            );
        }

        #[rstest]
        fn image_in_paragraph(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("Before ![photo](url.png) after"),
                Text::from(Line::from_iter([
                    Span::from("Before "),
                    Span::styled("[img] ", IMAGE_STYLE),
                    Span::styled("photo", IMAGE_STYLE),
                    Span::from(" after"),
                ]))
            );
        }

        #[rstest]
        fn multiple_images_in_paragraph(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("![first](first.png) and ![second](second.png)").to_string(),
                "[img] first and [img] second"
            );
        }

        #[rstest]
        fn formatted_alt_text_keeps_prefix_before_content(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("![**bold**](image.png)"),
                Text::from(Line::from_iter([
                    Span::styled("[img] ", IMAGE_STYLE),
                    Span::styled("bold", IMAGE_STYLE.bold()),
                ]))
            );
        }

        #[rstest]
        fn inline_code_counts_as_alt_text(_with_tracing: DefaultGuard) {
            let code_style = IMAGE_STYLE.patch(DefaultStyleSheet.code());
            assert_eq!(
                from_str("![`code`](image.png)"),
                Text::from(Line::from_iter([
                    Span::styled("[img] ", IMAGE_STYLE),
                    Span::styled("code", code_style),
                ]))
            );
        }

        #[rstest]
        fn marker_and_alt_compose_with_enclosing_style(_with_tracing: DefaultGuard) {
            let style = IMAGE_STYLE.bold();
            assert_eq!(
                from_str("**![diagram](diagram.png)**"),
                Text::from(Line::from_iter([
                    Span::styled("[img] ", style),
                    Span::styled("diagram", style),
                ]))
            );
        }

        #[rstest]
        fn marker_and_url_compose_with_enclosing_style(_with_tracing: DefaultGuard) {
            let style = IMAGE_STYLE.bold();
            assert_eq!(
                from_str("**![](diagram.png)**"),
                Text::from(Line::from_iter([
                    Span::styled("[img] ", style),
                    Span::styled("diagram.png", style),
                ]))
            );
        }

        #[rstest]
        fn inline_math_counts_as_alt_text(_with_tracing: DefaultGuard) {
            let math_style = IMAGE_STYLE.magenta();
            assert_eq!(
                from_str("![$x$](equation.png)"),
                Text::from(Line::from_iter([
                    Span::styled("[img] ", IMAGE_STYLE),
                    Span::styled("$x$", math_style),
                ]))
            );
        }

        #[rstest]
        fn inline_html_counts_as_alt_text(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("![<br>](break.png)"),
                Text::from(Line::from_iter([
                    Span::styled("[img] ", IMAGE_STYLE),
                    Span::styled("<br>", IMAGE_STYLE),
                ]))
            );
        }

        #[rstest]
        fn nested_image_description_preserves_order_and_style(_with_tracing: DefaultGuard) {
            let code_style = IMAGE_STYLE.patch(DefaultStyleSheet.code());
            assert_eq!(
                from_str("![outer ![inner](inner.png) `code`](outer.png)"),
                Text::from(Line::from_iter([
                    Span::styled("[img] ", IMAGE_STYLE),
                    Span::styled("outer ", IMAGE_STYLE),
                    Span::styled("[img] ", IMAGE_STYLE),
                    Span::styled("inner", IMAGE_STYLE),
                    Span::styled(" ", IMAGE_STYLE),
                    Span::styled("code", code_style),
                ]))
            );
        }

        #[rstest]
        fn empty_description_and_destination_omit_trailing_space(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("Before ![]() after"),
                Text::from(Line::from_iter([
                    Span::raw("Before "),
                    Span::styled("[img]", IMAGE_STYLE),
                    Span::raw(" after"),
                ]))
            );
        }

        #[rstest]
        fn multiline_description_renders_as_styled_inline_text(_with_tracing: DefaultGuard) {
            let markdown = indoc! {"
                ![first line
                second line](image.png)
            "};
            assert_eq!(
                from_str(markdown),
                Text::from(Line::from_iter([
                    Span::styled("[img] ", IMAGE_STYLE),
                    Span::styled("first line", IMAGE_STYLE),
                    Span::styled(" ", IMAGE_STYLE),
                    Span::styled("second line", IMAGE_STYLE),
                ]))
            );
        }

        #[rstest]
        fn hard_break_in_description_stays_inline(_with_tracing: DefaultGuard) {
            let markdown = indoc! {"
                ![first line  
                second line](image.png)
            "};
            assert_eq!(
                from_str(markdown),
                Text::from(Line::from_iter([
                    Span::styled("[img] ", IMAGE_STYLE),
                    Span::styled("first line", IMAGE_STYLE),
                    Span::styled(" ", IMAGE_STYLE),
                    Span::styled("second line", IMAGE_STYLE),
                ]))
            );
        }

        #[rstest]
        fn image_fallback_stays_inside_table_cell(_with_tracing: DefaultGuard) {
            let markdown = indoc! {"
                | Image |
                |-------|
                | ![photo](photo.png) |
            "};
            let rendered = from_str(markdown)
                .lines
                .iter()
                .map(ToString::to_string)
                .collect_vec();
            assert_eq!(
                rendered,
                [
                    "┌─────────────┐",
                    "│ Image       │",
                    "├─────────────┤",
                    "│ [img] photo │",
                    "└─────────────┘",
                ]
            );
        }

        #[rstest]
        fn url_fallback_uses_destination(_with_tracing: DefaultGuard) {
            let options = Options::default().image_fallback(ImageFallback::Url);
            assert_eq!(
                from_str_with_options("![diagram](diagram.png)", &options),
                Text::from(Line::from_iter([
                    Span::styled("[img] ", IMAGE_STYLE),
                    Span::styled("diagram.png", IMAGE_STYLE),
                ]))
            );
        }

        #[rstest]
        fn url_fallback_discards_complete_formatted_description(_with_tracing: DefaultGuard) {
            let options = Options::default().image_fallback(ImageFallback::Url);
            assert_eq!(
                from_str_with_options("![**bold** $x$ <br> `code`](diagram.png)", &options)
                    .to_string(),
                "[img] diagram.png"
            );
        }

        #[rstest]
        fn alt_text_and_url_fallback_preserves_formatted_description(_with_tracing: DefaultGuard) {
            let options = Options::default().image_fallback(ImageFallback::AltTextAndUrl);
            assert_eq!(
                from_str_with_options("![**diagram**](diagram.png)", &options),
                Text::from(Line::from_iter([
                    Span::styled("[img] ", IMAGE_STYLE),
                    Span::styled("diagram", IMAGE_STYLE.bold()),
                    Span::styled(" (diagram.png)", IMAGE_STYLE),
                ]))
            );
        }

        #[rstest]
        fn alt_text_and_url_fallback_uses_destination_for_empty_description(
            _with_tracing: DefaultGuard,
        ) {
            let options = Options::default().image_fallback(ImageFallback::AltTextAndUrl);
            assert_eq!(
                from_str_with_options("![](diagram.png)", &options).to_string(),
                "[img] diagram.png"
            );
        }

        #[rstest]
        fn configured_fallback_omits_space_when_destination_is_empty(
            #[values(ImageFallback::Url, ImageFallback::AltTextAndUrl)] fallback: ImageFallback,
            _with_tracing: DefaultGuard,
        ) {
            let options = Options::default().image_fallback(fallback);
            assert_eq!(
                from_str_with_options("![]()", &options),
                Text::from(Line::from(Span::styled("[img]", IMAGE_STYLE)))
            );
        }

        #[rstest]
        fn unstyled_fallback_does_not_modify_surrounding_text(_with_tracing: DefaultGuard) {
            let options = Options::new(UnstyledImageStyleSheet);
            assert_eq!(
                from_str_with_options("Before ![photo](photo.png) after", &options),
                Text::from(Line::from_iter([
                    Span::raw("Before "),
                    Span::raw("[img] "),
                    Span::raw("photo"),
                    Span::raw(" after"),
                ]))
            );
        }
    }

    #[cfg(feature = "highlight-code")]
    mod code_theme {
        use pretty_assertions::assert_eq;

        use super::*;

        #[rstest]
        fn different_theme_produces_different_output(_with_tracing: DefaultGuard) {
            let input = indoc! {"
                ```rust
                fn main() {}
                ```
            "};
            let default_out = from_str(input);
            let theme = CodeTheme::builtin(BuiltinCodeTheme::InspiredGitHub);
            let options = Options::default().code_theme(theme);
            let custom_out = from_str_with_options(input, &options);

            assert_ne!(default_out, custom_out);
        }

        #[rstest]
        fn explicit_default_theme_matches_implicit_default(_with_tracing: DefaultGuard) {
            let input = indoc! {"
                ```rust
                fn main() {}
                ```
            "};
            let implicit = from_str(input);
            let options = Options::default().code_theme(CodeTheme::default());
            let explicit = from_str_with_options(input, &options);

            assert_eq!(explicit, implicit);
        }
    }
}
