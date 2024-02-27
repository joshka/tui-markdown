#![allow(
    unused,
    dead_code,
    unused_variables,
    unused_imports,
    unused_mut,
    unused_assignments
)]
use std::io::{self};

use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Tag, TagEnd};
use ratatui::{prelude::*, symbols::line};
use tracing::{debug, info};

pub fn from_str<'a>(input: &'a str) -> Text<'a> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = pulldown_cmark::Parser::new_ext(input, options);
    from_events(parser)
}

pub fn from_events<'a, Iter>(events: Iter) -> Text<'a>
where
    Iter: Iterator<Item = Event<'a>>,
{
    let text = Text::default();
    let mut writer = TextWriter::new(events, text);
    writer.run();
    writer.text
}

struct TextWriter<'a, I> {
    /// Iterator supplying events.
    iter: I,

    /// Text to write to.
    text: Text<'a>,

    /// Current line.
    line: Option<Line<'a>>,

    /// Current style.
    style: Style,

    /// Current list index as a stack of indices.
    list_index: Vec<u64>,
}

impl<'a, I> TextWriter<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    fn new(iter: I, text: Text<'a>) -> Self {
        Self {
            iter,
            text,
            line: None,
            style: Style::default(),
            list_index: vec![],
        }
    }

    fn run(&mut self) {
        debug!("Running text writer");
        while let Some(event) = self.iter.next() {
            debug!("Event: {:?}", event);
            match event {
                Event::Start(tag) => self.start_tag(tag),
                Event::End(tag) => self.end_tag(tag),
                Event::Text(text) => self.text(text),
                Event::Code(code) => todo!(),
                Event::Html(html) => todo!(),
                Event::InlineHtml(html) => todo!(),
                Event::FootnoteReference(_) => todo!(),
                Event::SoftBreak => self.soft_break(),
                Event::HardBreak => self.hard_break(),
                Event::Rule => todo!(),
                Event::TaskListMarker(_) => todo!(),
            };
        }
    }

    fn start_tag(&mut self, tag: Tag<'a>) -> io::Result<()> {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading {
                level,
                id,
                classes,
                attrs,
            } => {
                self.start_heading(level);
            }
            Tag::BlockQuote => self.start_blockquote(),
            Tag::CodeBlock(_) => todo!(),
            Tag::HtmlBlock => todo!(),
            Tag::List(start_index) => {
                // TODO handle unordered lists properly (start_index is None for unordered lists)
                self.start_list(start_index.unwrap_or(0));
            }
            Tag::Item => {
                let width = self.list_index.len() * 4 - 3;
                if let Some(index) = self.list_index.last_mut() {
                    let span =
                        Span::styled(format!("{:width$}. ", index), Style::new().light_blue());
                    self.line = Some(span.into());
                    *index += 1;
                }
            }
            Tag::FootnoteDefinition(_) => todo!(),
            Tag::Table(_) => todo!(),
            Tag::TableHead => todo!(),
            Tag::TableRow => todo!(),
            Tag::TableCell => todo!(),
            Tag::Emphasis => self.style = self.style.italic(),
            Tag::Strong => self.style = self.style.bold(),
            Tag::Strikethrough => self.style = self.style.crossed_out(),
            Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            } => todo!(),
            Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            } => todo!(),
            Tag::MetadataBlock(_) => todo!(),
        }
        Ok(())
    }

    fn end_tag(&mut self, tag: TagEnd) -> io::Result<()> {
        match tag {
            TagEnd::Paragraph => {
                if let Some(line) = self.line.take() {
                    self.text.lines.push(line);
                    self.text.lines.push(Line::raw(""));
                }
            }
            TagEnd::Heading(_) => {
                if let Some(line) = self.line.take() {
                    self.text.lines.push(line);
                    self.text.lines.push(Line::raw(""));
                }
            }
            TagEnd::BlockQuote => {
                if let Some(line) = self.line.take() {
                    self.text.lines.push(line);
                    self.text.lines.push(Line::raw(""));
                }
            }
            TagEnd::CodeBlock => todo!(),
            TagEnd::HtmlBlock => todo!(),
            TagEnd::List(is_ordered) => {
                if let Some(line) = self.line.take() {
                    self.text.lines.push(line);
                }
                self.text.lines.push(Line::raw(""));
                self.list_index.pop();
            }
            TagEnd::Item => {
                if let Some(line) = self.line.take() {
                    self.text.lines.push(line);
                }
            }
            TagEnd::FootnoteDefinition => todo!(),
            TagEnd::Table => todo!(),
            TagEnd::TableHead => todo!(),
            TagEnd::TableRow => todo!(),
            TagEnd::TableCell => todo!(),
            TagEnd::Emphasis => self.style = self.style.not_italic(),
            TagEnd::Strong => self.style = self.style.not_bold(),
            TagEnd::Strikethrough => self.style = self.style.not_crossed_out(),
            TagEnd::Link => todo!(),
            TagEnd::Image => todo!(),
            TagEnd::MetadataBlock(_) => todo!(),
        }
        Ok(())
    }

    fn start_heading(&mut self, level: HeadingLevel) {
        let style = match level {
            HeadingLevel::H1 => Style::new().on_cyan().bold().underlined(),
            HeadingLevel::H2 => Style::new().cyan().bold(),
            HeadingLevel::H3 => Style::new().cyan().bold().italic(),
            HeadingLevel::H4 => Style::new().light_cyan().italic(),
            HeadingLevel::H5 => Style::new().light_cyan().italic(),
            HeadingLevel::H6 => Style::new().light_cyan().italic(),
        };
        let level_index = level as usize;
        let prefix = "#".repeat(level_index);
        self.line = Some(Line::styled(format!("{} ", prefix), style));
        debug!(?self.line, "Start heading");
    }

    fn start_blockquote(&mut self) {
        let span = Span::styled("> ", Style::new().green());
        self.line = Some(span.into());
        debug!(?self.line, "Start blockquote");
    }

    fn text(&mut self, text: CowStr<'a>) -> io::Result<()> {
        let span = Span::styled(text, self.style);
        if let Some(mut line) = self.line.take() {
            line.spans.push(span);
            self.line = Some(line);
        } else {
            self.line = Some(span.into());
        }
        debug!(?self.line, "Text");
        Ok(())
    }

    fn hard_break(&mut self) -> io::Result<()> {
        debug!("Newline");
        if let Some(line) = self.line.take() {
            self.text.lines.push(line);
        }
        Ok(())
    }

    fn start_list(&mut self, index: u64) {
        self.list_index.push(index);
    }

    fn soft_break(&mut self) -> io::Result<()> {
        let span = Span::styled(" ", self.style);
        if let Some(mut line) = self.line.take() {
            line.spans.push(span);
            self.line = Some(line);
        } else {
            self.line = Some(span.into());
        }
        Ok(())
    }
}
