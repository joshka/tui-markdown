use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use clap::builder::styling::AnsiColor;
use clap::builder::Styles;
use clap::Parser;
use color_eyre::eyre::{eyre, Ok, WrapErr};
use color_eyre::Result;
use tracing::{debug, info, Level};

use crate::app::App;
use crate::events::Events;

mod app;
mod events;
mod logging;

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let log_events = logging::init_logger(Level::DEBUG)?;

    let args = Cli::parse();
    let path = args.path;
    let events = Events::new()?;
    info!("Reading file {:?}", path);
    let markdown = read_file(&path)?;
    let text = tui_markdown::from_str(&markdown);
    let _height = text.height();
    let app = App::new(text, &path, events, log_events);
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
#[command(author, version, about, styles = HELP_STYLES)]
struct Cli {
    /// The path to the markdown file to read
    #[arg(default_value = "README.md")]
    path: PathBuf,
}
