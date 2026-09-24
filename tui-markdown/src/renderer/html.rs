//! Raw Markdown HTML rendering.
//!
//! HTML remains visible as literal text. Inline tags compose with enclosing formatting, while HTML
//! blocks preserve their physical lines and surrounding block spacing.

use pulldown_cmark::{CowStr, Event};
use ratatui_core::text::{Line, Span};

use super::TextWriter;
use crate::StyleSheet;

impl<'a, 'theme, I, S> TextWriter<'a, 'theme, I, S>
where
    I: Iterator<Item = Event<'a>>,
    S: StyleSheet,
{
    pub fn start_html_block(&mut self) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        self.push_line(Line::default());
        self.line_styles.push(self.styles.html());
        self.needs_newline = false;
    }

    pub fn end_html_block(&mut self) {
        self.line_styles.pop();
        self.needs_newline = true;
    }

    pub fn html_block(&mut self, html: CowStr<'a>) {
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

    pub fn inline_html(&mut self, html: CowStr<'a>) {
        let inline_style = self.inline_styles.last().copied().unwrap_or_default();
        let style = inline_style.patch(self.styles.html());
        self.push_span(Span::styled(html, style));
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use ratatui_core::style::Stylize;
    use ratatui_core::text::{Line, Span, Text};
    use rstest::rstest;

    use crate::from_str;
    use crate::renderer::test_support::{with_tracing, DefaultGuard};

    mod html {
        use super::*;
        use pretty_assertions::assert_eq;

        #[rstest]
        fn inline_html_tag(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("Hello <em>world</em>"),
                Text::from(Line::from_iter([
                    Span::from("Hello "),
                    Span::from("<em>").dim(),
                    Span::from("world"),
                    Span::from("</em>").dim()
                ]))
            );
        }

        #[rstest]
        fn inline_html_combines_with_emphasis(_with_tracing: DefaultGuard) {
            assert_eq!(
                from_str("*Hello <em>world</em>*"),
                Text::from(Line::from_iter([
                    Span::from("Hello ").italic(),
                    Span::from("<em>").dim().italic(),
                    Span::from("world").italic(),
                    Span::from("</em>").dim().italic()
                ]))
            );
        }

        #[rstest]
        fn html_block_preserves_paragraph_spacing(_with_tracing: DefaultGuard) {
            insta::assert_debug_snapshot!(from_str(indoc!("
                Before

                <div>
                Custom HTML
                </div>

                After
            ")), @r#"
            Text::from_iter([
                Line::from("Before"),
                Line::default(),
                Line::from(Span::from("<div>").dim()),
                Line::from(Span::from("Custom HTML").dim()).dim(),
                Line::from(Span::from("</div>").dim()).dim(),
                Line::default(),
                Line::from("After"),
            ])
            "#);
        }
    }
}
