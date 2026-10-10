use pulldown_cmark::{Event, Parser};
use rstest::rstest;
use tui_markdown::{from_str, from_str_with_options, Options, StyleSheet};

#[derive(Clone, Copy)]
struct HiddenCodeFences;

impl StyleSheet for HiddenCodeFences {
    fn code_block_fence(&self) -> &str {
        ""
    }
}

#[rstest]
#[case::plain("not-a-language")]
#[case::unlabelled("")]
#[case::highlighted("rust")]
fn code_block_preserves_literal_lines(
    #[case] language: &str,
    #[values("\n", "\r\n")] newline: &str,
) {
    let options = Options::new(HiddenCodeFences);
    let source = format!(
        "Before{newline}{newline}```{language}{newline}let first = 1;{newline}{newline}let last = 2;{newline}```{newline}{newline}After"
    );
    let text = from_str_with_options(&source, &options);
    assert_eq!(
        text.to_string(),
        "Before\n\nlet first = 1;\n\nlet last = 2;\n\nAfter"
    );
    assert_eq!(
        text,
        from_str_with_options(&source.replace("\r\n", "\n"), &options)
    );
}

#[rstest]
#[case::empty("", "")]
#[case::blank_line("\n", "")]
#[case::leading_blank("\nfirst\n", "\nfirst")]
#[case::trailing_blank("first\n\n", "first\n")]
#[case::multiple_blank_lines("first\n\n\nlast\n", "first\n\n\nlast")]
fn plain_code_keeps_empty_line_positions(
    #[case] body: &str,
    #[case] expected: &str,
    #[values("\n", "\r\n")] newline: &str,
) {
    let options = Options::new(HiddenCodeFences);
    let source = format!("```not-a-language\n{body}```").replace('\n', newline);
    assert_eq!(
        from_str_with_options(&source, &options).to_string(),
        expected
    );
}

#[rstest]
#[case::plain("not-a-language")]
#[case::highlighted("rust")]
fn unclosed_code_keeps_the_final_line(
    #[case] language: &str,
    #[values("\n", "\r\n")] newline: &str,
) {
    let options = Options::new(HiddenCodeFences);
    let source = format!("```{language}{newline}let first = 1;{newline}let last = 2;");
    let text = from_str_with_options(&source, &options);
    assert_eq!(text.to_string(), "let first = 1;\nlet last = 2;");
    assert_eq!(
        text,
        from_str_with_options(&source.replace("\r\n", "\n"), &options)
    );
}

#[rstest]
fn html_block_preserves_literal_lines(#[values("\n", "\r\n")] newline: &str) {
    let lf = "Before\n\n<pre>\nfirst\n\nlast\n</pre>\n\nAfter";
    let source = lf.replace('\n', newline);
    let text = from_str(&source);
    assert_eq!(text.to_string(), lf);
    assert_eq!(text, from_str(lf));
}

#[test]
fn metadata_line_endings_keep_the_same_spans() {
    let lf = "---\nname: example\nitems:\n  - one\n---\n\nAfter";
    let crlf = lf.replace('\n', "\r\n");
    assert_eq!(from_str(&crlf), from_str(lf));
}

#[rstest]
fn indented_code_preserves_its_blank_line(#[values("\n", "\r\n")] newline: &str) {
    let options = Options::new(HiddenCodeFences);
    let source = "Before\n\n    first\n\n    last\n\nAfter".replace('\n', newline);
    assert_eq!(
        from_str_with_options(&source, &options).to_string(),
        "Before\n\nfirst\n\nlast\n\nAfter"
    );
}

#[cfg(feature = "highlight-code")]
#[rstest]
#[case::comment("/* first\n\nlast */\n")]
#[case::raw_string("let value = r#\"first\n\nlast\"#;\n")]
fn highlighted_code_keeps_multiline_state(
    #[case] body: &str,
    #[values("\n", "\r\n")] newline: &str,
) {
    let options = Options::new(HiddenCodeFences);
    let lf = format!("```rust\n{body}```");
    let source = lf.replace('\n', newline);
    let text = from_str_with_options(&source, &options);
    assert_eq!(text.to_string(), body.strip_suffix('\n').unwrap());
    assert_eq!(text, from_str_with_options(&lf, &options));
}

#[rstest]
#[case::paragraphs("first\nsecond\n\n**third**")]
#[case::hard_break("first  \nsecond")]
#[case::list("- first\n- **second**")]
#[case::table("| Name | Value |\n| --- | --- |\n| first | **second** |")]
#[case::inline_html("Before <em>first</em>\nsecond")]
fn ordinary_markdown_keeps_lf_crlf_output(#[case] lf: &str) {
    let crlf = lf.replace('\n', "\r\n");
    assert_eq!(from_str(&crlf), from_str(lf));
}

#[rstest]
#[case::literal_spaces("first  second")]
#[case::lf("first\nsecond")]
#[case::crlf("first\r\nsecond")]
fn inline_code_preserves_the_parsers_space_normalization(#[case] code: &str) {
    let source = format!("Before `{code}` after");
    let parsed = Parser::new(&source)
        .find_map(|event| match event {
            Event::Code(code) => Some(code),
            _ => None,
        })
        .expect("the fixture contains an inline code span");
    assert_eq!(
        from_str(&source).to_string(),
        format!("Before {parsed} after")
    );
}
