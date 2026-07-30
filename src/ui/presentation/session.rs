// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt::Display;
use std::io::{self, BufRead, Write};

use vauchi_app::ui::AppEngine;
use vauchi_core::{Command, Event};

use super::{PresentationState, prompt_event, render_to_string};

pub trait CommandReducer {
    type Error: Display;

    fn initial_commands(&mut self) -> Result<Vec<Command>, Self::Error>;
    fn dispatch(&mut self, event: Event) -> Result<Vec<Command>, Self::Error>;
}

impl CommandReducer for AppEngine {
    type Error = vauchi_app::ui::AppPresentationError;

    fn initial_commands(&mut self) -> Result<Vec<Command>, Self::Error> {
        self.initial_commands()
    }

    fn dispatch(&mut self, event: Event) -> Result<Vec<Command>, Self::Error> {
        self.dispatch(event)
    }
}

pub fn run_engine(engine: &mut AppEngine) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_with_io(engine, &mut stdin.lock(), &mut stdout.lock()).map_err(Into::into)
}

pub fn run_with_io(
    reducer: &mut impl CommandReducer,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> io::Result<()> {
    let mut state = PresentationState::default();
    let mut commands = reducer.initial_commands().map_err(reducer_error)?;
    loop {
        let effects = state.apply(&commands);
        if state.native_back_requested() {
            return Ok(());
        }
        if let Some(event) = execute_effects(&effects, input, output)? {
            commands = reducer.dispatch(event).map_err(reducer_error)?;
            continue;
        }
        write!(output, "{}", render_to_string(&state))?;
        output.flush()?;
        let event = prompt_event(&state, input, output)?;
        commands = reducer.dispatch(event).map_err(reducer_error)?;
    }
}

fn reducer_error(error: impl Display) -> io::Error {
    io::Error::other(format!("Core reducer rejected event: {error}"))
}

fn execute_effects(
    effects: &[Command],
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> io::Result<Option<Event>> {
    for effect in effects {
        match effect {
            Command::PresentAlert { alert } => {
                writeln!(output, "  {}: {}", alert.title, alert.message)?;
            }
            Command::ShowToast { toast } => {
                writeln!(output, "  {}", toast.message)?;
            }
            Command::OpenExternalUrl { url } => {
                writeln!(output, "  Open: {url}")?;
            }
            Command::PostNotification { notification } => {
                writeln!(output, "  {}: {}", notification.title, notification.body)?;
            }
            Command::QrRequestScan => {
                write!(output, "  QR data > ")?;
                output.flush()?;
                let mut data = String::new();
                input.read_line(&mut data)?;
                return Ok(Some(Event::QrScanned {
                    data: data.trim().to_string(),
                }));
            }
            Command::FilePickFromUser { .. } => {
                write!(output, "  File path (blank to cancel) > ")?;
                output.flush()?;
                let mut path = String::new();
                input.read_line(&mut path)?;
                let path = path.trim();
                if path.is_empty() {
                    return Ok(Some(Event::FilePickCancelledByUser));
                }
                let bytes = std::fs::read(path)?;
                let filename = std::path::Path::new(path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_string();
                return Ok(Some(Event::FilePickedFromUser { bytes, filename }));
            }
            Command::ResetApplication => {
                writeln!(output, "  Application state reset.")?;
            }
            unsupported => {
                return Ok(Some(Event::HardwareUnavailable {
                    transport: unsupported.variant_name().to_string(),
                }));
            }
        }
    }
    Ok(None)
}
