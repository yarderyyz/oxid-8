use clap::Parser;

use std::{
    fs::File,
    io::{self, Read},
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    files: Vec<String>,
}

fn read_file(filename: &str) -> Result<String, io::Error> {
    let mut file = File::open(filename)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

fn main() {
    let args = Args::parse();
    args.files.iter().for_each(|asm| print!("{asm:}"));

    println!("CHIP-8 ASM Compiler");
}
