use std::io::{stdout, Stdout};

use color_eyre::{eyre::Context, Result};
use crossterm::{execute, terminal::*};
use ratatui::prelude::*;

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal
pub fn init() -> Result<Tui> {
    execute!(stdout(), EnterAlternateScreen).wrap_err("Could not enter alternate screen")?;
    enable_raw_mode().wrap_err("Could not enable raw mode")?;
    Terminal::new(CrosstermBackend::new(stdout())).wrap_err("Could not initialize terminal")
}

/// Restore the terminal to its original state
pub fn restore() -> Result<()> {
    execute!(stdout(), LeaveAlternateScreen).wrap_err("Could not leave alternate screen")?;
    disable_raw_mode().wrap_err("Could not disable raw mode")
}

/// A scope method for the terminal that restores the terminal to its original state
pub fn scoped<F>(f: F) -> Result<()>
where
    F: FnOnce(&mut Tui) -> Result<()>,
{
    let mut terminal = init()?;
    f(&mut terminal)?;
    restore()
}
