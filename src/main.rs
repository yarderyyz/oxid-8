use clap::Parser;
use std::fs::File;
use std::io::{self, Read};

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

fn main() {
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

    chip.run();
}
