//! Markdown event rendering.
//!
//! [`TextWriter`] owns the event loop and shared output state. The event matches remain here as an
//! index of supported pulldown-cmark events, while each Markdown construct keeps its state,
//! rendering behavior, and tests in the corresponding child module.
//!
//! Inline handlers ultimately write spans through [`TextWriter::push_span`]. Active image
//! descriptions receive those spans first, followed by active table cells, then the output line.
//! This sink order preserves inline event ordering inside buffered constructs.

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;
use std::vec;

use itertools::Itertools;
use pulldown_cmark::{CowStr, Event, Options as ParseOptions, Parser, Tag, TagEnd};
use ratatui_core::style::Style;
use ratatui_core::text::{Line, Span, Text};
use tracing::{debug, instrument};

#[cfg(feature = "highlight-code")]
use crate::code_theme::CodeTheme;
use crate::options::{ImageFallback, Options};
use crate::style_sheet::StyleSheet;
use crate::RenderContext;

mod blockquote;
mod code;
mod definition_list;
mod footnote;
mod formatting;
mod heading;
mod html;
mod image;
mod link;
mod list;
mod math;
mod table;
#[cfg(test)]
mod test_support;

/// Render Markdown `input` into a [`Text`] using the default [`Options`].
///
/// The returned text may borrow from `input`. Image syntax renders as a styled text fallback; this
/// function does not read or render image resources.
///
/// # Example
///
/// ```
/// use tui_markdown::from_str;
///
/// let text = from_str("# Status\n\nReady");
///
/// assert_eq!(text.to_string(), "# Status\n\nReady");
/// ```
pub fn from_str(input: &str) -> Text<'_> {
    from_str_with_options(input, &Options::default())
}

/// Render Markdown `input` into a [`Text`] using the supplied [`Options`].
///
/// The returned text may borrow from `input`. The options control styles, image fallback content,
/// and, with the `highlight-code` feature, fenced-code syntax highlighting.
///
/// # Example
///
/// ```
/// use tui_markdown::{from_str_with_options, ImageFallback, Options};
///
/// let options = Options::default().image_fallback(ImageFallback::AltTextAndUrl);
/// let text = from_str_with_options("![diagram](diagram.png)", &options);
///
/// assert_eq!(text.to_string(), "[img] diagram (diagram.png)");
/// ```
pub fn from_str_with_options<'a, S>(input: &'a str, options: &Options<S>) -> Text<'a>
where
    S: StyleSheet,
{
    render(input, options, None)
}

fn render<'a, S: StyleSheet>(
    input: &'a str,
    options: &Options<S>,
    context: Option<RenderContext>,
) -> Text<'a> {
    let parser = Parser::new_ext(input, parser_options());

    let mut writer = TextWriter::new(parser, options.styles.clone(), options.image_fallback);
    writer.context = context;
    #[cfg(feature = "highlight-code")]
    let writer = writer.with_code_theme(options.selected_code_theme());
    writer.run()
}

fn parser_options() -> ParseOptions {
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
    parse_opts
}

/// Renders Markdown with an explicit terminal body width and table-presentation limits.
///
/// Unlike [`from_str_with_options`], this opt-in entry point prepares visual rows for the supplied
/// context. Application-owned reply markers are not included in that width. Zero width returns no
/// rows without parsing. Graphemes wider than the whole body use the context's ASCII replacement;
/// ordinary line-end overflow wraps to the next row.
pub fn from_str_with_context<'a, S: StyleSheet>(
    input: &'a str,
    options: &Options<S>,
    context: &RenderContext,
) -> Text<'a> {
    if context.width() == 0 {
        return Text::default();
    }
    crate::layout::wrap_text(render(input, options, Some(*context)), *context)
}

pub(crate) struct RenderPrefix {
    pub text: Text<'static>,
    pub needs_newline: bool,
    pub table_fallbacks: [usize; 3],
    pub table_count: usize,
}

pub(crate) struct RenderCheckpoint {
    pub source_offset: usize,
    pub semantic_row: usize,
    pub needs_newline: bool,
    pub table_fallbacks: [usize; 3],
    pub table_count: usize,
}

pub(crate) struct StreamingRender {
    pub text: Text<'static>,
    pub checkpoint: Option<RenderCheckpoint>,
    pub events: u64,
    pub blocks: u64,
    pub has_global_dependency: bool,
    pub table_fallbacks: [usize; 3],
    pub table_count: usize,
}

struct TrackedRender<'a> {
    text: Text<'a>,
    checkpoint: Option<RenderCheckpoint>,
    events: u64,
    blocks: u64,
    has_global_dependency: bool,
    table_fallbacks: [usize; 3],
    table_count: usize,
}

#[derive(Default)]
struct TrackingState {
    depth: usize,
    pending_start: Option<usize>,
    previous_block_start: Option<usize>,
    previous_block_was_table: bool,
    events: u64,
    blocks: u64,
    has_global_dependency: bool,
}

struct TrackingEvents<'a, I> {
    iter: I,
    source: &'a str,
    base_offset: usize,
    state: Rc<RefCell<TrackingState>>,
}

impl<'a, I> Iterator for TrackingEvents<'a, I>
where
    I: Iterator<Item = (Event<'a>, Range<usize>)>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let (event, range) = self.iter.next()?;
        let mut state = self.state.borrow_mut();
        state.events += 1;
        match &event {
            Event::Start(tag) => {
                if matches!(tag, Tag::FootnoteDefinition(_)) {
                    state.has_global_dependency = true;
                }
                if state.depth == 0 && is_block_tag(tag) {
                    let block_start = line_start(self.source, range.start);
                    let can_advance = state.previous_block_start.is_none_or(|start| {
                        !state.previous_block_was_table
                            || has_blank_line(&self.source[start..block_start])
                    });
                    if can_advance {
                        state.pending_start = Some(self.base_offset + block_start);
                    }
                    state.previous_block_start = Some(block_start);
                    state.previous_block_was_table = matches!(tag, Tag::Table(_));
                    state.blocks += 1;
                }
                state.depth += 1;
            }
            Event::End(_) => {
                state.depth = state.depth.saturating_sub(1);
            }
            Event::FootnoteReference(_) => state.has_global_dependency = true,
            Event::Rule if state.depth == 0 => {
                let block_start = line_start(self.source, range.start);
                if self.base_offset + block_start == 0 {
                    // A leading thematic rule can become a YAML metadata opener after later input.
                    state.has_global_dependency = true;
                }
                let can_advance = state.previous_block_start.is_none_or(|start| {
                    !state.previous_block_was_table
                        || has_blank_line(&self.source[start..block_start])
                });
                if can_advance {
                    state.pending_start = Some(self.base_offset + block_start);
                }
                state.previous_block_start = Some(block_start);
                state.previous_block_was_table = false;
                state.blocks += 1;
            }
            _ => {}
        }
        drop(state);
        Some(event)
    }
}

fn has_blank_line(gap: &str) -> bool {
    gap.split_inclusive('\n').any(|line| {
        let Some(line) = line.strip_suffix('\n') else {
            return false;
        };
        let line = line.strip_suffix('\r').unwrap_or(line);
        line.bytes().all(|byte| matches!(byte, b' ' | b'\t'))
    })
}

fn line_start(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())]
        .rfind('\n')
        .map_or(0, |newline| newline + 1)
}

fn is_block_tag(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::HtmlBlock
            | Tag::List(_)
            | Tag::FootnoteDefinition(_)
            | Tag::Table(_)
            | Tag::MetadataBlock(_)
            | Tag::DefinitionList
    )
}

pub(crate) fn render_streaming<S: StyleSheet>(
    input: &str,
    options: &Options<S>,
    context: RenderContext,
    base_offset: usize,
    prefix: RenderPrefix,
) -> StreamingRender {
    let unresolved = Rc::new(Cell::new(false));
    let callback_flag = Rc::clone(&unresolved);
    let parser = Parser::new_with_broken_link_callback(
        input,
        parser_options(),
        Some(move |_| {
            callback_flag.set(true);
            None
        }),
    );
    let has_reference_definitions = parser.reference_definitions().iter().next().is_some();
    let state = Rc::new(RefCell::new(TrackingState {
        has_global_dependency: has_reference_definitions,
        ..TrackingState::default()
    }));
    let events = TrackingEvents {
        iter: parser.into_offset_iter(),
        source: input,
        base_offset,
        state: Rc::clone(&state),
    };
    let mut writer = TextWriter::with_prefix(
        events,
        options.styles.clone(),
        options.image_fallback,
        prefix.text,
        prefix.needs_newline,
    );
    writer.table_fallbacks = prefix.table_fallbacks;
    writer.table_count = prefix.table_count;
    writer.context = Some(context);
    #[cfg(feature = "highlight-code")]
    let writer = writer.with_code_theme(options.selected_code_theme());
    let tracked = writer.run_tracked(&state);
    StreamingRender {
        text: own_text(tracked.text),
        checkpoint: tracked.checkpoint,
        events: tracked.events,
        blocks: tracked.blocks,
        has_global_dependency: tracked.has_global_dependency || unresolved.get(),
        table_fallbacks: tracked.table_fallbacks,
        table_count: tracked.table_count,
    }
}

fn own_text(text: Text<'_>) -> Text<'static> {
    Text {
        alignment: text.alignment,
        style: text.style,
        lines: text
            .lines
            .into_iter()
            .map(|line| Line {
                style: line.style,
                alignment: line.alignment,
                spans: line
                    .spans
                    .into_iter()
                    .map(|span| Span::styled(span.content.into_owned(), span.style))
                    .collect(),
            })
            .collect(),
    }
}

struct TextWriter<'a, 'theme, I, S: StyleSheet> {
    // Core output state.
    /// Iterator supplying Markdown events.
    iter: I,
    /// Rendered terminal text.
    text: Text<'a>,
    /// Styles for nested inline constructs, with the active style at the top.
    inline_styles: Vec<Style>,
    /// Prefixes added to each output line, from the outermost block to the innermost.
    line_prefixes: Vec<Span<'a>>,
    /// Styles for nested line-oriented constructs, with the active style at the top.
    line_styles: Vec<Style>,
    /// The [`StyleSheet`] used to style the output.
    styles: S,
    /// Whether the next block needs to start on a new line.
    needs_newline: bool,
    /// Whether raw text is inside a metadata block.
    in_metadata_block: bool,
    /// Optional width-aware layout; absent for the original batch API.
    context: Option<RenderContext>,

    // Code rendering state.
    /// Active syntax highlighter while rendering a recognized fenced code block.
    #[cfg(feature = "highlight-code")]
    code_highlighter: Option<syntect::easy::HighlightLines<'theme>>,
    /// Explicit theme used when a fenced code block starts highlighting.
    ///
    /// When absent, code highlighting resolves the shared built-in default.
    #[cfg(feature = "highlight-code")]
    code_theme: Option<&'theme CodeTheme>,
    /// Keeps the writer's shape consistent when syntax highlighting is disabled.
    #[cfg(not(feature = "highlight-code"))]
    code_theme_lifetime: std::marker::PhantomData<&'theme ()>,

    // Heading rendering state.
    /// Heading attributes to append after heading content.
    heading_meta: Option<heading::HeadingMeta<'a>>,

    // Link rendering state.
    /// A link which will be appended to the current line when the link tag is closed.
    link: Option<CowStr<'a>>,

    // Image rendering state.
    /// Images whose descriptions are currently being collected.
    images: Vec<image::PendingImage<'a>>,
    /// Content to render in place of images.
    image_fallback: ImageFallback,

    // List rendering state.
    /// Current list index as a stack of indices.
    list_indices: Vec<Option<u64>>,
    /// Layout of each active list item, from the outermost item to the innermost.
    list_items: Vec<list::ListItemLayout>,

    // Paragraph-suppression state.
    /// Whether we are inside a footnote definition.
    in_footnote_definition: bool,
    /// Whether we are inside a definition-list description.
    in_definition_description: bool,

    // Table rendering state.
    /// Active table builder that accumulates cells during table parsing.
    table_builder: Option<table::TableBuilder<'a>>,
    /// Current snapshot's table fallback counts by [`table::TableFallback`] discriminant.
    table_fallbacks: [usize; 3],
    /// Number of tables retained in the current renderer snapshot.
    table_count: usize,
}

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
            styles,
            needs_newline: false,
            in_metadata_block: false,
            context: None,
            #[cfg(feature = "highlight-code")]
            code_highlighter: None,
            #[cfg(feature = "highlight-code")]
            code_theme: None,
            #[cfg(not(feature = "highlight-code"))]
            code_theme_lifetime: std::marker::PhantomData,
            heading_meta: None,
            link: None,
            images: vec![],
            image_fallback,
            list_indices: vec![],
            list_items: vec![],
            in_footnote_definition: false,
            in_definition_description: false,
            table_builder: None,
            table_fallbacks: [0; 3],
            table_count: 0,
        }
    }

    fn with_prefix(
        iter: I,
        styles: S,
        image_fallback: ImageFallback,
        text: Text<'a>,
        needs_newline: bool,
    ) -> Self {
        let mut writer = Self::new(iter, styles, image_fallback);
        writer.text = text;
        writer.needs_newline = needs_newline;
        writer
    }

    fn run(mut self) -> Text<'a> {
        debug!("Running text writer");
        while let Some(event) = self.iter.next() {
            self.handle_event(event);
        }
        self.text
    }

    fn run_tracked(mut self, state: &Rc<RefCell<TrackingState>>) -> TrackedRender<'a> {
        debug!("Running tracked text writer");
        let mut checkpoint = None;
        while let Some(event) = self.iter.next() {
            if let Some(source_offset) = state.borrow_mut().pending_start.take() {
                checkpoint = Some(RenderCheckpoint {
                    source_offset,
                    semantic_row: self.text.lines.len(),
                    needs_newline: self.needs_newline,
                    table_fallbacks: self.table_fallbacks,
                    table_count: self.table_count,
                });
            }
            self.handle_event(event);
        }
        let state = state.borrow();
        TrackedRender {
            text: self.text,
            checkpoint,
            events: state.events,
            blocks: state.blocks,
            has_global_dependency: state.has_global_dependency,
            table_fallbacks: self.table_fallbacks,
            table_count: self.table_count,
        }
    }

    #[instrument(level = "debug", skip_all)]
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
            } => self.start_heading(level, heading::HeadingMeta { id, classes, attrs }),
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
        // Loose list items emit a paragraph start after the item handler has already written the
        // marker. Keep only that first paragraph on the marker line; later paragraphs have either
        // added content or set `needs_newline`.
        let list_marker_line_is_open = self.list_items.last().is_some_and(|item| {
            !self.needs_newline
                && self.text.lines.len() == item.marker_line + 1
                && self.text.lines[item.marker_line].spans.len() == item.marker_span_count
        });
        if list_marker_line_is_open {
            return;
        }

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
        self.needs_newline = true;
    }

    fn text(&mut self, text: CowStr<'a>) {
        if self.table_builder.is_some() {
            let style = self.inline_styles.last().copied().unwrap_or_default();
            self.push_span(Span::styled(text, style));
            return;
        }

        if self.push_highlighted_text(&text) {
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

    fn soft_break(&mut self) {
        if self.in_metadata_block {
            self.hard_break();
        } else if self.images.is_empty() {
            self.push_span(Span::raw(" "));
        } else {
            self.image_description_break();
        }
    }

    #[instrument(level = "trace", skip_all)]
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

    #[instrument(level = "trace", skip_all)]
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
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::test_support::{with_tracing, DefaultGuard};
    use super::*;

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
    fn paragraph_hard_break(_with_tracing: DefaultGuard) {
        let markdown = indoc! {r"
            Hello\
            World
        "};

        assert_eq!(from_str(markdown), Text::from_iter(["Hello", "World"]));
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
}
