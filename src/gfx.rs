use std::sync::atomic::Ordering;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Row, Table};
use ratatui::{style::Color, Frame};

use crate::consts::{PROGRAM_START, WINDOW};
use crate::cpu::{Chip8, Resolution};
use crate::decode::decode;

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
        .constraints([Constraint::Length(8), Constraint::Min(3)])
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
        Constraint::Length(14),
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
                let op = decode(u16::from_be_bytes([b, s]));
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

    let res_factor: usize = if matches!(chip.resolution, Resolution::High) {
        2
    } else {
        1
    };
    let [left_area, right_area] = Layout::horizontal([
        Constraint::Length((64 * res_factor as u16) + 2),
        Constraint::Percentage(60),
    ])
    .areas(main_area);

    let outer_left_block = Block::bordered().title("Oxid-8");
    let inner_left = outer_left_block.inner(left_area);

    frame.render_widget(outer_left_block, left_area);
    if debug {
        render_chip8_debug(frame, right_area, chip);
    }

    let buf = frame.buffer_mut();
    for y in 0..(16 * res_factor) {
        for x in 0..(8 * res_factor) {
            let mut fg = chip.screen[(y * 2, x)];
            let mut bg = chip.screen[((y * 2) + 1, x)];

            let x_buf = (x * 8) as u16 + inner_left.x;
            let y_buf = y as u16 + inner_left.y;

            for bit in 0..8 {
                if let Some(cell) = buf.cell_mut((x_buf + (8 - bit), y_buf)) {
                    cell.set_symbol("▀");
                    cell.set_fg(Color::Black);
                    cell.set_bg(Color::Black);
                    if fg & 0x1 == 0x1 {
                        cell.set_fg(Color::Blue);
                    }
                    if bg & 0x1 == 0x1 {
                        cell.set_bg(Color::Blue);
                    }
                }
                fg >>= 1;
                bg >>= 1;
            }
        }
    }
}
