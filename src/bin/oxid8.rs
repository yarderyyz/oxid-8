use clap::Parser;

use oxid8::audio::Beeper;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};

use std::fs::File;
use std::io::{self, Read};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use oxid8::consts::PROGRAM_START;
use oxid8::consts::RAM_SIZE;
use oxid8::cpu::{BufChannel, Chip8};
use oxid8::{gfx, timers};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    rom: String,
    #[arg(short, long)]
    debug: bool,
}

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
    running_state: RunningState,
}

#[derive(Debug, Default, PartialEq, Eq)]
enum RunningState {
    #[default]
    Running,
    Done,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Message {
    KeyDown(u8), // 0x0..=0xF
    KeyUp(u8),
    Quit,
}

fn main() -> color_eyre::Result<()> {
    let mut model = Model::default();

    let args = Args::parse();

    // TODO: Support other pc start points
    let mut chip = Chip8 {
        pc: PROGRAM_START,
        ..Chip8::default()
    };
    chip.load_font();

    let res = load_rom(&args.rom, &mut chip.memory[PROGRAM_START..]);
    if res.is_err() {
        panic!("Failed to load rom");
    }

    tui::install_panic_hook();
    let mut terminal = tui::init_terminal()?;

    let beeper = Beeper::new().unwrap();
    let timer_rx = timers::spawn_timers(chip.dt.clone(), chip.st.clone());

    // Setup async rendering thread using a BufChannel for communication.
    let (mut buf_tx, buf_rx) = BufChannel::new();
    thread::spawn(move || loop {
        while let Ok(screen) = buf_rx.try_recv() {
            if let Ok(screen) = screen.read() {
                // Render the current view
                terminal
                    .draw(|f| gfx::view(&screen, f, args.debug))
                    .unwrap();
            }
        }
        thread::sleep(Duration::from_nanos(16_666_667)); // ~60 Hz
    });

    let (input_tx, input_rx) = mpsc::channel::<Message>();
    thread::spawn(move || loop {
        // Handle events and map to a Message
        let message = if let Event::Key(key) = event::read().unwrap() {
            handle_key(key)
        } else {
            None
        };
        if let Some(message) = message {
            input_tx.send(message).unwrap();
        }

        thread::sleep(Duration::from_nanos(16_666_667)); // ~60 Hz
    });

    while model.running_state != RunningState::Done {
        chip.run_step();

        buf_tx.send(&chip.screen);

        // Run input
        while let Ok(message) = input_rx.try_recv() {
            match message {
                Message::KeyDown(key) => chip.press_key(key),
                Message::KeyUp(key) => chip.release_key(key),
                _ => {}
            }
            update(&mut model, message);
        }

        // Play sounds
        while let Ok(on) = timer_rx.try_recv() {
            beeper.set(on);
        }

        thread::sleep(Duration::from_millis(2)); // 500 Hz
    }

    tui::restore_terminal()?;
    Ok(())
}

fn chip8_key_of_char(c: char) -> Option<u8> {
    match c {
        '1' => Some(0x1),
        '2' => Some(0x2),
        '3' => Some(0x3),
        '4' => Some(0xC),
        'q' | 'Q' => Some(0x4),
        'w' | 'W' => Some(0x5),
        'e' | 'E' => Some(0x6),
        'r' | 'R' => Some(0xD),
        'a' | 'A' => Some(0x7),
        's' | 'S' => Some(0x8),
        'd' | 'D' => Some(0x9),
        'f' | 'F' => Some(0xE),
        'z' | 'Z' => Some(0xA),
        'x' | 'X' => Some(0x0),
        'c' | 'C' => Some(0xB),
        'v' | 'V' => Some(0xF),
        _ => None,
    }
}

fn handle_key(key: event::KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char(c) => {
            let k = chip8_key_of_char(c)?;
            match key.kind {
                KeyEventKind::Press | KeyEventKind::Repeat => Some(Message::KeyDown(k)),
                KeyEventKind::Release => Some(Message::KeyUp(k)),
            }
        }
        KeyCode::Esc => Some(Message::Quit),
        _ => None,
    }
}

fn update(model: &mut Model, msg: Message) -> Option<Message> {
    if let Message::Quit = msg {
        model.running_state = RunningState::Done;
    }
    None
}

mod tui {
    use ratatui::{
        backend::{Backend, CrosstermBackend},
        crossterm::{
            event::{
                KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
            },
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
        stdout().execute(PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES,
        ))?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        Ok(terminal)
    }

    pub fn restore_terminal() -> color_eyre::Result<()> {
        stdout().execute(LeaveAlternateScreen)?;
        let _ = stdout().execute(PopKeyboardEnhancementFlags);
        disable_raw_mode()?;
        Ok(())
    }

    pub fn install_panic_hook() {
        let original_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            stdout().execute(LeaveAlternateScreen).unwrap();
            let _ = stdout().execute(PopKeyboardEnhancementFlags);
            disable_raw_mode().unwrap();
            original_hook(panic_info);
        }));
    }
}
