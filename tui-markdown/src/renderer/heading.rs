//! Markdown heading rendering.

use pulldown_cmark::{CowStr, Event, HeadingLevel};
use ratatui_core::text::{Line, Span};

use super::TextWriter;
use crate::StyleSheet;

/// Heading attributes collected from pulldown-cmark to render after the heading text.
pub struct HeadingMeta<'a> {
    pub id: Option<CowStr<'a>>,
    pub classes: Vec<CowStr<'a>>,
    pub attrs: Vec<(CowStr<'a>, Option<CowStr<'a>>)>,
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

    /// Formats the attributes as a Markdown attribute block suffix.
    fn to_suffix(&self) -> Option<String> {
        let mut parts = Vec::new();

        if let Some(id) = &self.id {
            parts.push(format!("#{id}"));
        }
        for class in &self.classes {
            parts.push(format!(".{class}"));
        }
        for (key, value) in &self.attrs {
            match value {
                Some(value) => parts.push(format!("{key}={value}")),
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

impl<'a, 'theme, I, S> TextWriter<'a, 'theme, I, S>
where
    I: Iterator<Item = Event<'a>>,
    S: StyleSheet,
{
    pub fn start_heading(&mut self, level: HeadingLevel, heading_meta: HeadingMeta<'a>) {
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

    pub fn end_heading(&mut self) {
        if let Some(meta) = self.heading_meta.take() {
            if let Some(suffix) = meta.to_suffix() {
                self.push_span(Span::styled(suffix, self.styles.heading_meta()));
            }
        }
        self.needs_newline = true;
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;
    use crate::renderer::test_support::{with_tracing, DefaultGuard};
    use crate::renderer::*;

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
}
