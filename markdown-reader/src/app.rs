use std::path::Path;

use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
    StatefulWidgetRef, Widget, Wrap,
};
use ratatui::DefaultTerminal;

use crate::events::{CrosstermEvent, Event, Events};
use crate::logging::LogEvents;

#[derive(Debug)]
pub struct App<'a> {
    text: Text<'a>,
    path: &'a Path,
    events: Events,
    log_events: LogEvents,
    show_logs: bool,
}

impl<'a> App<'a> {
    pub fn new(text: Text<'a>, path: &'a Path, events: Events, log_events: LogEvents) -> App<'a> {
        App {
            text,
            path,
            events,
            log_events,
            show_logs: false,
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let mut state = ScrollState::new(self.text.height());
        self.draw(&mut terminal, &mut state)?;
        while let Ok(event) = self.events.next() {
            match event {
                Event::Crossterm(event) => self.handle_crossterm(event, &mut state)?,
                Event::Exit => break,
            };
            self.draw(&mut terminal, &mut state)?;
        }
        Ok(())
    }

    fn handle_crossterm(&mut self, event: CrosstermEvent, state: &mut ScrollState) -> Result<()> {
        match event {
            CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => {
                self.handle_key(key, state);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent, state: &mut ScrollState) {
        use KeyCode::*;
        match (key.modifiers, key.code) {
            (_, Char('q') | Esc) | (KeyModifiers::CONTROL, Char('c')) => {
                self.events.send(Event::Exit)
            }
            (_, Char('k') | Up) => state.scroll_up(),
            (_, Char('j') | Down) => state.scroll_down(),
            (_, Char('g') | Home) => state.scroll_top(),
            (_, Char('G') | End) => state.scroll_bottom(),
            (_, Char('b') | PageUp) | (KeyModifiers::SHIFT, Char(' ')) => state.scroll_page_up(),
            (_, Char('f') | PageDown) | (KeyModifiers::NONE, Char(' ')) => state.scroll_page_down(),
            (_, Char('l')) => self.toggle_logs(),
            _ => {}
        }
    }

    fn draw(&self, terminal: &mut DefaultTerminal, state: &mut ScrollState) -> Result<()> {
        terminal.draw(|frame| {
            frame.render_stateful_widget_ref(self, frame.area(), state);
        })?;
        Ok(())
    }

    fn toggle_logs(&mut self) {
        self.show_logs = !self.show_logs
    }
}

/// necessary as ScrollbarState fields are private
#[derive(Debug, Clone, Copy)]
pub struct ScrollState {
    pub position: usize,
    pub view_size: usize,
    pub max: usize,
}

impl ScrollState {
    fn new(max: usize) -> ScrollState {
        ScrollState {
            position: 0,
            view_size: 1,
            max,
        }
    }

    fn scroll_down(&mut self) {
        self.position = self.position.saturating_add(1);
    }

    fn scroll_up(&mut self) {
        self.position = self.position.saturating_sub(1);
    }

    fn scroll_page_down(&mut self) {
        self.position = self.position.saturating_add(self.view_size);
    }

    fn scroll_page_up(&mut self) {
        self.position = self.position.saturating_sub(self.view_size);
    }

    fn scroll_top(&mut self) {
        self.position = 0;
    }

    fn scroll_bottom(&mut self) {
        self.position = self.max.saturating_sub(self.view_size);
    }
}

impl From<&mut ScrollState> for ScrollbarState {
    fn from(state: &mut ScrollState) -> ScrollbarState {
        ScrollbarState::new(state.max.saturating_sub(state.view_size)).position(state.position)
    }
}

impl From<&mut ScrollState> for ListState {
    fn from(state: &mut ScrollState) -> ListState {
        ListState::default().with_selected(Some(state.position))
    }
}

impl StatefulWidgetRef for &App<'_> {
    type State = ScrollState;
    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let logs_height = if self.show_logs { 1 } else { 0 };
        let [header, body, log] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(2),
            Constraint::Fill(logs_height),
        ])
        .areas(area);

        let [body, scrollbar] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]).areas(body);
        state.view_size = body.height as usize;
        // state.position = state
        //     .position
        //     .min(self.text.height().saturating_sub(state.view_size));
        let header_line = Line::from(vec![
            Span::raw("File: "),
            Span::styled(self.path.to_string_lossy(), (Color::White, Modifier::BOLD)),
        ]);
        Paragraph::new(header_line).render(header, buf);
        let position = state
            .position
            .min(self.text.height().saturating_sub(state.view_size)) as u16;
        Paragraph::new(self.text.clone())
            .scroll((position, 0))
            .wrap(Wrap { trim: false })
            .render(body, buf);
        let mut scrollbar_state = state.into();
        Scrollbar::new(ScrollbarOrientation::VerticalRight).render(
            scrollbar,
            buf,
            &mut scrollbar_state,
        );

        let mut list_state = state.into();
        self.log_events.render_ref(log, buf, &mut list_state);
    }
}
