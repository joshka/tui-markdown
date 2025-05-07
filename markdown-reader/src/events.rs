use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

use color_eyre::eyre::Context;
use color_eyre::Result;
use ratatui::crossterm::event;

pub type CrosstermEvent = ratatui::crossterm::event::Event;

pub enum Event {
    Crossterm(CrosstermEvent),
    Exit,
}

#[derive(Debug)]
pub struct Events {
    pub event_rx: Receiver<Event>,
    pub event_tx: Sender<Event>,
}

impl Events {
    pub fn new() -> Result<Events> {
        let (event_tx, event_rx) = channel();
        let crossterm_tx = event_tx.clone();
        thread::spawn(move || poll_crossterm_events(crossterm_tx));
        Ok(Events { event_rx, event_tx })
    }

    pub fn send(&self, event: Event) {
        self.event_tx.send(event).unwrap();
    }

    pub fn next(&self) -> Result<Event> {
        self.event_rx.recv().wrap_err("Done receiving events")
    }
}

fn poll_crossterm_events(event_tx: Sender<Event>) {
    loop {
        if event::poll(Duration::from_millis(100)).unwrap() {
            if let Ok(event) = event::read() {
                event_tx.send(Event::Crossterm(event)).unwrap();
            }
        }
    }
}
