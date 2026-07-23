//! Markdown definition-list rendering.
//!
//! Terms and descriptions have separate styles. Each description starts with `: `, and paragraph
//! handling preserves blank lines inside multi-paragraph descriptions.

use pulldown_cmark::Event;
use ratatui_core::text::{Line, Span};

use super::TextWriter;
use crate::StyleSheet;

impl<'a, 'theme, I, S> TextWriter<'a, 'theme, I, S>
where
    I: Iterator<Item = Event<'a>>,
    S: StyleSheet,
{
    pub fn start_definition_list(&mut self) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        self.needs_newline = false;
    }

    pub fn end_definition_list(&mut self) {
        self.needs_newline = true;
    }

    pub fn start_definition_title(&mut self) {
        // Definition-list terms contain inline events without an ordinary paragraph start, so the
        // term handler owns the output line and applies the term style before consuming them.
        self.push_line(Line::default());
        self.push_inline_style(self.styles.definition_term());
        self.needs_newline = false;
    }

    pub fn end_definition_title(&mut self) {
        self.pop_inline_style();
        self.needs_newline = false;
    }

    pub fn start_definition_description(&mut self) {
        // A tight description contains inline events without an ordinary paragraph start. Create
        // its line here and write the visible Markdown `: ` marker before consuming the content so
        // the marker and content share the description style.
        self.push_line(Line::default());
        self.push_span(Span::styled(": ", self.styles.definition_description()));
        self.push_inline_style(self.styles.definition_description());
        self.in_definition_description = true;
        self.needs_newline = false;
    }

    pub fn end_definition_description(&mut self) {
        self.pop_inline_style();
        self.in_definition_description = false;
        self.needs_newline = false;
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::renderer::test_support::{with_tracing, DefaultGuard};
    use crate::renderer::*;
    use crate::*;

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
}
