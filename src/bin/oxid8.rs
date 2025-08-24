use clap::Parser;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};

use atomic_enum::atomic_enum;

use std::fs::File;
use std::io::{self, Read};
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use oxid8::audio::Beeper;
use oxid8::consts::{H, PROGRAM_START, RAM_SIZE, W};
use oxid8::cpu::{Chip8, Screen};
use oxid8::triple_buffer;
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

#[atomic_enum]
#[derive(PartialEq, Eq)]
enum RunningState {
    Running = 0,
    Done,
}

#[derive(Debug)]
struct Model {
    running_state: Arc<AtomicRunningState>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Message {
    KeyDown(u8), // 0x0..=0xF
    KeyUp(u8),
    Quit,
}

fn main() -> color_eyre::Result<()> {
    let mut model = Model {
        running_state: Arc::new(AtomicRunningState::new(RunningState::Running)),
    };

    let args = Args::parse();

    let mut chip = Chip8::new();
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
    let (mut buf_tx, buf_rx) = triple_buffer::triple_buffer::<Chip8>(Chip8::new());
    let running_state = model.running_state.clone();
    let render_join_handle = thread::spawn(move || {
        while running_state.load(Ordering::Acquire) != RunningState::Done {
            {
                let read_handle = buf_rx.read();
                // Render the current view
                terminal
                    .draw(|f| gfx::view(&read_handle, f, args.debug))
                    .unwrap();
            }
            thread::sleep(Duration::from_nanos(16_666_667)); // ~60 Hz
        }
    });

    let (input_tx, input_rx) = mpsc::channel::<Message>();
    let running_state = model.running_state.clone();
    let input_join_handle = thread::spawn(move || {
        while running_state.load(Ordering::Acquire) != RunningState::Done {
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
        }
    });

    while model.running_state.load(Ordering::Acquire) != RunningState::Done {
        chip.run_step();

        {
            let mut send_handle = buf_tx.write();
            *send_handle = chip.clone(); // must clone here as screen is causal
        }

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

    let _ = render_join_handle.join();
    let _ = input_join_handle.join();

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
        model
            .running_state
            .store(RunningState::Done, Ordering::Release);
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
            style::Print,
            terminal::{
                disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement,
                EnterAlternateScreen, LeaveAlternateScreen,
            },
            ExecutableCommand,
        },
        Terminal,
    };
    use std::{io::stdout, panic};

    pub fn init_terminal() -> color_eyre::Result<Terminal<impl Backend>> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;

        // Check if kitty keyboard protocol is supported before enabling
        if supports_keyboard_enhancement()? {
            // Then set enhancement flags
            stdout().execute(PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES,
            ))?;
        } else {
            panic!("Terminal must support kitty keyboard enhancements");
        }

        let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        Ok(terminal)
    }

    pub fn restore_terminal() -> color_eyre::Result<()> {
        // Disable kitty keyboard protocol with CSI < u
        stdout().execute(PopKeyboardEnhancementFlags)?;
        stdout().execute(LeaveAlternateScreen)?;
        disable_raw_mode()?;
        Ok(())
    }

    pub fn install_panic_hook() {
        let original_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            stdout().execute(PopKeyboardEnhancementFlags).unwrap();
            stdout().execute(LeaveAlternateScreen).unwrap();
            disable_raw_mode().unwrap();
            original_hook(panic_info);
        }));
    }
}
