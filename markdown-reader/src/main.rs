use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use clap::builder::styling::AnsiColor;
use clap::builder::Styles;
use clap::{Parser, ValueEnum};
use color_eyre::eyre::{eyre, Ok, WrapErr};
use color_eyre::Result;
use tracing::{debug, info, Level};
use tui_markdown::{BuiltinCodeTheme, CodeTheme, ImageFallback, Options};

use crate::app::App;
use crate::events::Events;

mod app;
mod events;
mod logging;

fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Cli::parse();
    let options = args.renderer_options()?;
    let log_events = logging::init_logger(Level::DEBUG)?;
    info!("Reading file {:?}", args.path);
    let markdown = read_file(&args.path)?;
    let text = tui_markdown::from_str_with_options(&markdown, &options);
    let events = Events::new()?;

    // Keep startup errors out of the alternate screen and enter terminal mode only after every
    // fallible input and configuration step has completed.
    let terminal = ratatui::init();
    let app = App::new(text, &args.path, events, log_events);
    let result = app.run(terminal);
    ratatui::restore();
    result
}

fn read_file(path: &Path) -> Result<String> {
    debug!("Reading file {:?}", path);
    let input = File::open(path).wrap_err_with(|| eyre!("Could not open {:?}", path))?;
    let mut reader = BufReader::new(input);
    let mut buf = String::new();
    reader
        .read_to_string(&mut buf)
        .wrap_err("Could not read file")?;
    Ok(buf)
}

const HELP_STYLES: Styles = Styles::styled()
    .header(AnsiColor::Blue.on_default().bold())
    .usage(AnsiColor::Blue.on_default().bold())
    .literal(AnsiColor::White.on_default())
    .placeholder(AnsiColor::Green.on_default());

#[derive(Debug, Parser)]
#[command(name = "mdr", author, version, about, styles = HELP_STYLES)]
struct Cli {
    /// The path to the markdown file to read
    #[arg(default_value = "README.md")]
    path: PathBuf,

    /// Text to display in place of Markdown images
    #[arg(long, value_enum, value_name = "MODE", default_value = "alt-text")]
    image_fallback: ImageFallbackArg,

    /// Built-in syntax-highlighting theme (default: base16-ocean-dark)
    #[arg(
        long,
        value_enum,
        value_name = "THEME",
        conflicts_with = "code_theme_file"
    )]
    code_theme: Option<CodeThemeArg>,

    /// Load a custom syntax-highlighting theme from a TextMate .tmTheme file
    #[arg(long, value_name = "PATH", conflicts_with = "code_theme")]
    code_theme_file: Option<PathBuf>,
}

impl Cli {
    fn renderer_options(&self) -> Result<Options> {
        let options = Options::default().image_fallback(self.image_fallback.into());
        let options = if let Some(code_theme) = self.code_theme {
            options.code_theme(code_theme)
        } else if let Some(path) = &self.code_theme_file {
            options.code_theme(CodeTheme::from_file(path)?)
        } else {
            options
        };
        Ok(options)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum ImageFallbackArg {
    /// Display the image description, falling back to its URL
    AltText,
    /// Display the image URL
    Url,
    /// Display the image description and URL
    AltTextAndUrl,
}

impl From<ImageFallbackArg> for ImageFallback {
    fn from(image_fallback: ImageFallbackArg) -> Self {
        match image_fallback {
            ImageFallbackArg::AltText => Self::AltText,
            ImageFallbackArg::Url => Self::Url,
            ImageFallbackArg::AltTextAndUrl => Self::AltTextAndUrl,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum CodeThemeArg {
    /// The dark Base16 Eighties theme
    Base16EightiesDark,
    /// The dark Base16 Mocha theme
    Base16MochaDark,
    /// The default dark Base16 Ocean theme
    Base16OceanDark,
    /// The light Base16 Ocean theme
    Base16OceanLight,
    /// The light Inspired GitHub theme
    InspiredGithub,
    /// The dark Solarized theme
    SolarizedDark,
    /// The light Solarized theme
    SolarizedLight,
}

impl From<CodeThemeArg> for BuiltinCodeTheme {
    fn from(code_theme: CodeThemeArg) -> Self {
        match code_theme {
            CodeThemeArg::Base16EightiesDark => Self::Base16EightiesDark,
            CodeThemeArg::Base16MochaDark => Self::Base16MochaDark,
            CodeThemeArg::Base16OceanDark => Self::Base16OceanDark,
            CodeThemeArg::Base16OceanLight => Self::Base16OceanLight,
            CodeThemeArg::InspiredGithub => Self::InspiredGitHub,
            CodeThemeArg::SolarizedDark => Self::SolarizedDark,
            CodeThemeArg::SolarizedLight => Self::SolarizedLight,
        }
    }
}

impl From<CodeThemeArg> for CodeTheme {
    fn from(code_theme: CodeThemeArg) -> Self {
        BuiltinCodeTheme::from(code_theme).into()
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn feature_showcase() {
        let markdown = include_str!("../TEST.md");
        let text = tui_markdown::from_str(markdown);

        // The text snapshot makes whitespace and construct transitions easy to review. The debug
        // snapshot also records every line and span style, which catches formatting that leaks
        // across a construct boundary but leaves the visible characters unchanged.
        insta::assert_snapshot!("feature_showcase_text", text);
        insta::assert_debug_snapshot!("feature_showcase_styles", text);
    }

    #[test]
    fn image_fallback_modes_select_rendered_content() {
        let cases = [
            ("alt-text", "[img] diagram"),
            ("url", "[img] diagram.png"),
            ("alt-text-and-url", "[img] diagram (diagram.png)"),
        ];

        for (mode, expected) in cases {
            let cli = Cli::try_parse_from(["mdr", "--image-fallback", mode]).unwrap();
            let options = cli.renderer_options().unwrap();
            let text = tui_markdown::from_str_with_options("![diagram](diagram.png)", &options);
            assert_eq!(text.to_string(), expected);
        }
    }

    #[test]
    fn every_builtin_code_theme_is_accepted() {
        let themes = [
            "base16-eighties-dark",
            "base16-mocha-dark",
            "base16-ocean-dark",
            "base16-ocean-light",
            "inspired-github",
            "solarized-dark",
            "solarized-light",
        ];

        for theme in themes {
            let cli = Cli::try_parse_from(["mdr", "--code-theme", theme]).unwrap();
            let options = cli.renderer_options().unwrap();
            assert!(options.selected_code_theme().is_some());
        }
    }

    #[test]
    fn built_in_and_file_themes_conflict() {
        let result = Cli::try_parse_from([
            "mdr",
            "--code-theme",
            "solarized-dark",
            "--code-theme-file",
            "custom.tmTheme",
        ]);

        assert!(result.is_err());
    }

    #[test]
    fn theme_file_errors_include_the_requested_path() {
        let cli =
            Cli::try_parse_from(["mdr", "--code-theme-file", "missing-theme.tmTheme"]).unwrap();

        let error = cli.renderer_options().unwrap_err();

        assert!(error.to_string().contains("missing-theme.tmTheme"));
    }

    #[test]
    fn help() {
        insta::assert_snapshot!("help", Cli::command().render_long_help());
    }
}
