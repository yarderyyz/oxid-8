use clap::Parser;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::Backend;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use std::fs::File;
use std::io::{self, Read};
use std::sync::atomic::Ordering;
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
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
    #[arg(short, long)]
    print: bool,
    #[arg(short, long)]
    debug: bool,
}

const RAM_SIZE: usize = types::RAM_SIZE;
const PROGRAM_START: usize = types::PROGRAM_START;
const WINDOW: isize = types::WINDOW;

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

    if args.print {
        chip.memory[PROGRAM_START..PROGRAM_START + 100]
            .chunks(2)
            .for_each(|bs| {
                println!("{}", Chip8::parseop(u16::from_be_bytes([bs[0], bs[1]])));
            });
        return Ok(());
    }

    tui::install_panic_hook();
    let mut terminal = tui::init_terminal()?;

    {
        // Run a thread to decrement DT
        let dt = chip.dt.clone();
        let st = chip.dt.clone();
        thread::spawn(move || loop {
            let _ = dt.fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                if v > 0 {
                    Some(v - 1)
                } else {
                    None
                }
            });
            let st = st.fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                if v > 0 {
                    Some(v - 1)
                } else {
                    None
                }
            });
            sleep(Duration::from_secs_f64(1.0 / 60.0));
        });
    }

    while model.running_state != RunningState::Done {
        chip.run_step();

        // Render the current view
        terminal.draw(|f| view(&chip, f, args.debug))?;

        // Handle events and map to a Message
        let mut current_msg = handle_event(&model)?;

        // Process updates as long as they return a non-None message
        while current_msg.is_some() {
            if let Some(msg) = current_msg {
                match msg {
                    Message::KeyDown(key) => chip.press_key(key),
                    Message::KeyUp(key) => chip.release_key(key),
                    _ => {}
                }
            }
            current_msg = update(&mut model, current_msg.unwrap());
        }
    }

    tui::restore_terminal()?;
    Ok(())
}

pub fn render_chip8_debug(f: &mut Frame, area: Rect, c8: &Chip8) {
    // ── split the screen ────────────────────────────────────────────────────────
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Percentage(20),
            Constraint::Percentage(35),
        ])
        .split(area);

    // ── left-hand side: scalar regs + V-regs ────────────────────────────────────
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(3)])
        .split(chunks[0]);

    // ----- small scalar register table (PC / I / SP / DT / ST) -----
    let reg_rows = vec![
        Row::new(vec!["PC".into(), format!("0x{:03X}", c8.pc)]),
        Row::new(vec!["I".into(), format!("0x{:03X}", c8.i)]),
        Row::new(vec!["SP".into(), c8.sp.to_string()]),
        Row::new(vec!["DT".into(), c8.dt.load(Ordering::Acquire).to_string()]),
        Row::new(vec!["ST".into(), c8.st.load(Ordering::Acquire).to_string()]),
    ];
    let reg_widths = [Constraint::Length(4), Constraint::Length(12)];
    let reg_table = Table::new(reg_rows, reg_widths)
        .header(Row::new(vec!["Reg", "Value"]).style(Style::default().add_modifier(Modifier::BOLD)))
        .block(Block::default().borders(Borders::ALL).title("Registers"));
    f.render_widget(reg_table, left[0]);

    // ----- V0..VF in a 4×4 grid -----
    let mut v_rows = Vec::with_capacity(4);
    for g in 0..4 {
        let b = g * 4;
        v_rows.push(Row::new(vec![
            format!("V{:X}..V{:X}", b, b + 3),
            format!("0x{:02X}", c8.v[b]),
            format!("0x{:02X}", c8.v[b + 1]),
            format!("0x{:02X}", c8.v[b + 2]),
            format!("0x{:02X}", c8.v[b + 3]),
        ]));
    }
    let v_widths = [
        Constraint::Length(9),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(6),
    ];
    let v_table = Table::new(v_rows, v_widths)
        .header(
            Row::new(vec!["Group", "0", "1", "2", "3"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(Block::default().borders(Borders::ALL).title("V Registers"));
    f.render_widget(v_table, left[1]);

    // ── right-hand side: CHIP-8 keypad (pressed = green) ────────────────────────
    const MAP: [[(&str, u8); 4]; 4] = [
        [("1", 0x1), ("2", 0x2), ("3", 0x3), ("C", 0xC)],
        [("4", 0x4), ("5", 0x5), ("6", 0x6), ("D", 0xD)],
        [("7", 0x7), ("8", 0x8), ("9", 0x9), ("E", 0xE)],
        [("A", 0xA), ("0", 0x0), ("B", 0xB), ("F", 0xF)],
    ];

    let on = Style::default()
        .fg(Color::Black)
        .bg(Color::Green)
        .add_modifier(Modifier::BOLD);
    let off = Style::default();

    let key_rows: Vec<Row> = MAP
        .iter()
        .map(|row| {
            Row::new(
                row.iter()
                    .map(|(lbl, code)| {
                        Span::styled(*lbl, if c8.keys[*code as usize] { on } else { off })
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    let key_widths = [Constraint::Length(3); 4];
    let key_table = Table::new(key_rows, key_widths)
        .block(Block::default().borders(Borders::ALL).title("Keypad"));
    f.render_widget(key_table, chunks[1]);

    let cmd_widths = [
        Constraint::Length(7),
        Constraint::Length(12),
        Constraint::Length(6),
    ];
    let mut cmd_rows: Vec<Row> = Vec::with_capacity((WINDOW as usize * 2) + 1);

    let hilite = Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD);

    for d in -WINDOW..=WINDOW {
        let addr_isize = c8.pc as isize + d * 2;

        let mut row = if addr_isize < PROGRAM_START as isize {
            Row::new(vec!["-".to_string(), "-".into(), "-".into()])
        } else {
            let addr = addr_isize as usize;
            if addr + 1 >= c8.memory.len() {
                Row::new(vec!["-".to_string(), "-".into(), "-".into()])
            } else {
                let b = c8.memory[addr];
                let s = c8.memory[addr + 1];
                let op = Chip8::parseop(u16::from_be_bytes([b, s]));
                Row::new(vec![
                    format!("0x{addr:03X}"),
                    format!("{op}"),
                    format!("({op:?})"),
                ])
            }
        };

        if d == 0 {
            row = row.style(hilite);
        }
        cmd_rows.push(row);
    }

    let cmd_table = Table::new(cmd_rows, cmd_widths)
        .block(Block::default().borders(Borders::ALL).title("Instructions"));
    f.render_widget(cmd_table, chunks[2]);
}

pub fn view(chip: &Chip8, frame: &mut Frame, debug: bool) {
    let main_area = frame.area();

    let [left_area, right_area] =
        Layout::horizontal([Constraint::Length(68), Constraint::Percentage(60)]).areas(main_area);

    let outer_left_block = Block::bordered().title("Oxid-8");
    let inner_left = outer_left_block.inner(left_area);

    frame.render_widget(outer_left_block, left_area);
    //frame.render_widget(game_window.clone(), inner_left);

    //let text = game_window.debug.join("\n");
    render_chip8_debug(frame, right_area, chip);

    let buf = frame.buffer_mut();

    for y in 0..16 {
        for x in 0..8 {
            let mut fg = chip.screen[y * 2][x];
            let mut bg = chip.screen[(y * 2) + 1][x];

            let x_buf = (x * 8) as u16 + inner_left.x;
            let y_buf = y as u16 + inner_left.y;

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
            return Ok(handle_key(key));
        }
    }
    Ok(None)
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
