use color_eyre::Result;
use ratatui::{
    prelude::*,
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, WidgetRef},
};

use crate::{
    events::{CrosstermEvent, Event, Events},
    logging::LogEvents,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub struct App<'a> {
    text: Text<'a>,
    events: Events,
    log_events: LogEvents,
    scroll_position: usize,
    show_logs: bool,
}

impl<'a> App<'a> {
    pub fn new(text: Text<'a>, events: Events, log_events: LogEvents) -> App {
        App {
            text,
            events,
            log_events,
            scroll_position: 0,
            show_logs: false,
        }
    }

    pub fn run(mut self, terminal: &mut Terminal<impl Backend>) -> Result<()> {
        self.redraw(terminal)?;
        while let Ok(event) = self.events.next() {
            match event {
                Event::Crossterm(event) => self.handle_crossterm(event)?,
                Event::Exit => break,
            };
            self.redraw(terminal)?;
        }
        Ok(())
    }

    fn handle_crossterm(&mut self, event: CrosstermEvent) -> Result<()> {
        match event {
            CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => {
                self.handle_key(key);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        use KeyCode::*;
        match (key.modifiers, key.code) {
            (_, Char('q') | Esc) | (KeyModifiers::CONTROL, Char('c')) => {
                self.events.send(Event::Exit)
            }
            (_, Char('k') | Up) => self.scroll_up(),
            (_, Char('j') | Down) => self.scroll_down(),
            (_, Char('g') | Home) => self.scroll_top(),
            (_, Char('G') | End) => self.scroll_bottom(),
            (_, Char('l')) => self.toggle_logs(),
            _ => {}
        }
    }

    fn redraw(&self, terminal: &mut Terminal<impl Backend>) -> Result<()> {
        terminal.draw(|frame| frame.render_widget_ref(self, frame.size()))?;
        Ok(())
    }

    fn scroll_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        self.scroll_position = self.scroll_position.saturating_add(1);
    }

    fn scroll_top(&mut self) {
        self.scroll_position = 0
    }

    fn scroll_bottom(&mut self) {
        self.scroll_position = self.text.height()
    }

    fn toggle_logs(&mut self) {
        self.show_logs = !self.show_logs
    }
}

impl WidgetRef for &App<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let logs_height = if self.show_logs { 1 } else { 0 };
        let [header, body, log] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(2),
            Constraint::Fill(logs_height),
        ])
        .areas(area);

        let [body, scrollbar] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]).areas(body);

        "Markdown Reader".bold().render(header, buf);
        Paragraph::new(self.text.clone())
            .scroll((self.scroll_position as u16, 0))
            .render(body, buf);
        let mut scrollbar_state =
            ScrollbarState::new(self.text.height()).position(self.scroll_position);
        Scrollbar::new(ScrollbarOrientation::VerticalRight).render(
            scrollbar,
            buf,
            &mut scrollbar_state,
        );

        self.log_events.render(log, buf);
    }
}
