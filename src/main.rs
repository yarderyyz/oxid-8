use clap::Parser;
use std::fs::File;
use std::io::{self, Read};
use std::time::Duration;

use ratatui::{
    crossterm::event::{self, Event, KeyCode},
    style::Color,
    Frame,
};

use crate::types::Chip8;
mod chip8_tests;
mod types;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    rom: String,
}

const RAM_SIZE: usize = types::RAM_SIZE;
const PROGRAM_START: usize = types::PROGRAM_START;

fn load_rom(filename: &str, memory: &mut [u8]) -> io::Result<()> {
    let mut file = File::open(filename)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;
    if contents.len() > RAM_SIZE - PROGRAM_START {
        panic!("Rom too large");
    }
    memory[0..contents.len()].copy_from_slice(&contents[0..]);
    Ok(())
}

#[derive(Debug, Default)]
struct Model {
    counter: i32,
    running_state: RunningState,
}

#[derive(Debug, Default, PartialEq, Eq)]
enum RunningState {
    #[default]
    Running,
    Done,
}

#[derive(PartialEq)]
enum Message {
    Increment,
    Decrement,
    Reset,
    Quit,
}

fn main() -> color_eyre::Result<()> {
    tui::install_panic_hook();
    let mut terminal = tui::init_terminal()?;
    let mut model = Model::default();

    let args = Args::parse();

    // TODO: Support other pc start points
    let mut chip = Chip8 {
        pc: PROGRAM_START as u16,
        ..Chip8::default()
    };

    let res = load_rom(&args.rom, &mut chip.memory[PROGRAM_START..]);
    if res.is_err() {
        panic!("Failed to load rom");
    }

    while model.running_state != RunningState::Done {
        chip.run_step();

        // Render the current view
        terminal.draw(|f| view(&chip, f))?;

        // Handle events and map to a Message
        let mut current_msg = handle_event(&model)?;

        // Process updates as long as they return a non-None message
        while current_msg.is_some() {
            current_msg = update(&mut model, current_msg.unwrap());
        }
    }

    tui::restore_terminal()?;
    Ok(())
}

pub fn view(chip: &Chip8, frame: &mut Frame) {
    let buf = frame.buffer_mut();

    for y in 0..16 {
        for x in 0..8 {
            let mut fg = chip.screen[y * 2][x];
            let mut bg = chip.screen[(y * 2) + 1][x];

            let x_buf = (x * 8) as u16;
            let y_buf = y as u16;

            for bit in 0..8 {
                if let Some(cell) = buf.cell_mut((x_buf + (8 - bit), y_buf)) {
                    cell.set_symbol("▀");
                    cell.set_fg(Color::Black);
                    cell.set_bg(Color::Black);
                    if fg & 0x1 == 0x1 {
                        cell.set_fg(Color::White);
                    }
                    if bg & 0x1 == 0x1 {
                        cell.set_bg(Color::White);
                    }
                }
                fg >>= 1;
                bg >>= 1;
            }
        }
    }
}

/// Convert Event to Message
///
/// TODO: Make event handling async into a cue - for now this is the system
///       Frequency lol
fn handle_event(_: &Model) -> color_eyre::Result<Option<Message>> {
    if event::poll(Duration::from_millis(2))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press {
                return Ok(handle_key(key));
            }
        }
    }
    Ok(None)
}

fn handle_key(key: event::KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char('j') => Some(Message::Increment),
        KeyCode::Char('k') => Some(Message::Decrement),
        KeyCode::Char('q') => Some(Message::Quit),
        _ => None,
    }
}

fn update(model: &mut Model, msg: Message) -> Option<Message> {
    match msg {
        Message::Increment => {
            model.counter += 1;
            if model.counter > 50 {
                return Some(Message::Reset);
            }
        }
        Message::Decrement => {
            model.counter -= 1;
            if model.counter < -50 {
                return Some(Message::Reset);
            }
        }
        Message::Reset => model.counter = 0,
        Message::Quit => {
            // You can handle cleanup and exit here
            model.running_state = RunningState::Done;
        }
    };
    None
}

mod tui {
    use ratatui::{
        backend::{Backend, CrosstermBackend},
        crossterm::{
            terminal::{
                disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
            },
            ExecutableCommand,
        },
        Terminal,
    };
    use std::{io::stdout, panic};

    pub fn init_terminal() -> color_eyre::Result<Terminal<impl Backend>> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        Ok(terminal)
    }

    pub fn restore_terminal() -> color_eyre::Result<()> {
        stdout().execute(LeaveAlternateScreen)?;
        disable_raw_mode()?;
        Ok(())
    }

    pub fn install_panic_hook() {
        let original_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            stdout().execute(LeaveAlternateScreen).unwrap();
            disable_raw_mode().unwrap();
            original_hook(panic_info);
        }));
    }
}
