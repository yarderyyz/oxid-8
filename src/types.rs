use std::fmt;
/*
*   Memory Map:
*   +---------------+= 0xFFF (4095) End of Chip-8 RAM
*   |               |
*   |               |
*   |               |
*   |               |
*   |               |
*   | 0x200 to 0xFFF|
*   |     Chip-8    |
*   | Program / Data|
*   |     Space     |
*   |               |
*   |               |
*   |               |
*   +- - - - - - - -+= 0x600 (1536) Start of ETI 660 Chip-8 programs
*   |               |
*   |               |
*   |               |
*   +---------------+= 0x200 (512) Start of most Chip-8 programs
*   | 0x000 to 0x1FF|
*   | Reserved for  |
*   |  interpreter  |
*   +---------------+= 0x000 (0) Start of Chip-8 RAM
*
*/
pub const RAM_SIZE: usize = 4096;
pub const PROGRAM_START: usize = 0x200;

pub struct Memory(pub [u8; RAM_SIZE]);
impl Default for Memory {
    fn default() -> Self {
        Self([0; RAM_SIZE])
    }
}
impl std::ops::Deref for Memory {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for Memory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipOp {
    Cls,
    Ret,
    Jp { nnn: u16 },
    Call { nnn: u16 },
    Se { x: u8, kk: u8 },
    Sne { x: u8, kk: u8 },
    Ser { x: u8, y: u8 },
    Ld { x: u8, kk: u8 },
    Add { x: u8, kk: u8 },
    Ldr { x: u8, y: u8 },
    Orr { x: u8, y: u8 },
    Andr { x: u8, y: u8 },
    Xorr { x: u8, y: u8 },
    Addr { x: u8, y: u8 },
    Subr { x: u8, y: u8 },
    Shrr { x: u8, y: u8 },
    Subnr { x: u8, y: u8 },
    Shlr { x: u8, y: u8 },
    Sner { x: u8, y: u8 },
    Ldi { nnn: u16 },
    Jpo { nnn: u16 },
    Rnd { x: u8, kk: u8 },
    Drw { x: u8, y: u8, n: u8 },
    Skp { x: u8 },
    Sknp { x: u8 },
    Lddv { x: u8 },
    Ldk { x: u8 },
    Ldvd { x: u8 },
    Ldsv { x: u8 },
    Addi { x: u8 },
    Ldfv { x: u8 },
    Ldbv { x: u8 },
    Ldiv { x: u8 },
    Ldvi { x: u8 },
    Unknown(u16),
}

impl fmt::Display for ChipOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ChipOp::*;
        match *self {
            Cls => write!(f, "CLS"),
            Ret => write!(f, "RET"),
            Jp { nnn } => write!(f, "JP {:#05X}", nnn),
            Call { nnn } => write!(f, "CALL {:#05X}", nnn),
            Se { x, kk } => write!(f, "SE V{:X}, {:#04X}", x, kk),
            Sne { x, kk } => write!(f, "SNE V{:X}, {:#04X}", x, kk),
            Ser { x, y } => write!(f, "SE V{:X}, V{:X}", x, y),
            Ld { x, kk } => write!(f, "LD V{:X}, {:#04X}", x, kk),
            Add { x, kk } => write!(f, "ADD V{:X}, {:#04X}", x, kk),
            Ldr { x, y } => write!(f, "LD V{:X}, V{:X}", x, y),
            Orr { x, y } => write!(f, "OR V{:X}, V{:X}", x, y),
            Andr { x, y } => write!(f, "AND V{:X}, V{:X}", x, y),
            Xorr { x, y } => write!(f, "XOR V{:X}, V{:X}", x, y),
            Addr { x, y } => write!(f, "ADD V{:X}, V{:X}", x, y),
            Subr { x, y } => write!(f, "SUB V{:X}, V{:X}", x, y), // <-- e.g. SUB V1, V0
            Shrr { x, y } => write!(f, "SHR V{:X}, V{:X}", x, y),
            Subnr { x, y } => write!(f, "SUBN V{:X}, V{:X}", x, y),
            Shlr { x, y } => write!(f, "SHL V{:X}, V{:X}", x, y),
            Sner { x, y } => write!(f, "SNE V{:X}, V{:X}", x, y),
            Ldi { nnn } => write!(f, "LD I, {:#05X}", nnn),
            Jpo { nnn } => write!(f, "JP V0, {:#05X}", nnn),
            Rnd { x, kk } => write!(f, "RND V{:X}, {:#04X}", x, kk),
            Drw { x, y, n } => write!(f, "DRW V{:X}, V{:X}, {:#X}", x, y, n),
            Skp { x } => write!(f, "SKP V{:X}", x),
            Sknp { x } => write!(f, "SKNP V{:X}", x),
            Lddv { x } => write!(f, "LD V{:X}, DT", x),
            Ldk { x } => write!(f, "LD V{:X}, K", x),
            Ldvd { x } => write!(f, "LD DT, V{:X}", x),
            Ldsv { x } => write!(f, "LD ST, V{:X}", x),
            Addi { x } => write!(f, "ADD I, V{:X}", x),
            Ldfv { x } => write!(f, "LD F, V{:X}", x),
            Ldbv { x } => write!(f, "LD B, V{:X}", x),
            Ldiv { x } => write!(f, "LD [I], V{:X}", x),
            Ldvi { x } => write!(f, "LD V{:X}, [I]", x),
            Unknown(op) => write!(f, "DB {:#06X}", op),
        }
    }
}

const W: usize = 8;
const H: usize = 32;

#[allow(dead_code)]
#[derive(Default)]
pub struct Chip8 {
    pub pc: u16,     // Program counter
    pub v: [u8; 16], // General purpose registers
    pub i: u16,      // Address register
    pub sp: usize,   // Stack Pointer
    pub dt: u8,      // Delay timer
    pub st: u8,      // Sound timer
    pub stack: [u16; 16],
    pub screen: [[u8; W]; H],
    pub memory: Memory,
}

impl Chip8 {
    pub fn print_screen(&self) {
        for y in 0..H {
            for x in 0..W {
                print!("{:08b}", self.screen[y][x]);
            }
            println!();
        }
    }
    pub fn run_step(&mut self) {
        let pc = self.pc as usize;
        let b = self.memory[pc];
        let s = self.memory[pc + 1];
        let op = Chip8::parseop(u16::from_be_bytes([b, s]));
        self.run_op(op);
    }
    pub fn run<F: Fn(&Chip8)>(&mut self, render: F) {
        loop {
            self.run_step();
            render(self);
        }
    }
    pub fn run_op(&mut self, op: ChipOp) {
        use ChipOp::*;
        match op {
            Cls => {
                self.screen.fill([0; W]);
                self.pc += 2;
            }
            Ret => {
                self.pc = self.stack[self.sp - 1];
                self.sp -= 1;
            }
            Jp { nnn } => {
                self.pc = nnn;
            }
            Call { nnn } => {
                self.sp += 1;
                self.stack[self.sp - 1] = self.pc;
                self.pc = nnn;
            }
            Se { x, kk } => {
                if self.v[x as usize] == kk {
                    self.pc += 4;
                } else {
                    self.pc += 2;
                }
            }
            Sne { x, kk } => {
                if *self.vx(x) != kk {
                    self.pc += 4;
                } else {
                    self.pc += 2;
                }
            }
            Ser { x, y } => {
                if *self.vx(x) == *self.vx(y) {
                    self.pc += 4;
                } else {
                    self.pc += 2;
                }
            }
            Ld { x, kk } => {
                *self.vx(x) = kk;
                self.pc += 2;
            }
            Add { x, kk } => {
                let r = self.vx(x);
                *r = r.wrapping_add(kk);
                self.pc += 2;
            }
            Ldr { x, y } => {
                let vy = *self.vx(y);
                let vx = self.vx(x);
                *vx = vy;
                self.pc += 2;
            }
            Orr { .. }
            | Andr { .. }
            | Xorr { .. }
            | Addr { .. }
            | Subr { .. }
            | Shrr { .. }
            | Subnr { .. }
            | Shlr { .. }
            | Sner { .. } => {}
            Ldi { nnn } => {
                self.i = nnn;
                self.pc += 2;
            }
            Jpo { .. } | Rnd { .. } => {}
            Drw { x, y, n } => {
                let vx = self.v[x as usize] as usize;
                let vy = self.v[y as usize] as usize;
                let bit_off = vx & 7; // vx % 8
                let col_byte = vx >> 3; // vx / 8
                let i = self.i as usize;
                let height = n as usize;

                let rows = self.screen.len();
                let bytes_per_row = self.screen[0].len();

                // collision flag (VF)
                self.v[0xF] = 0;

                for (row, &byte) in self.memory[i..i + height].iter().enumerate() {
                    let y_idx = (vy + row) % rows;
                    let x0 = col_byte % bytes_per_row;
                    let x1 = (col_byte + 1) % bytes_per_row; // next byte (wrap horizontally)

                    // Shift the 8-bit sprite line by bit_off across two bytes.
                    let shifted = (u16::from(byte) << 8) >> bit_off;
                    let [hi, lo] = shifted.to_be_bytes();

                    // Cache low and hi bytes to check collision flag
                    let before0 = self.screen[y_idx][x0];
                    let before1 = self.screen[y_idx][x1];

                    self.screen[y_idx][x0] ^= hi;
                    self.screen[y_idx][x1] ^= lo;

                    // Check and set collision flag (VF)
                    if (before0 & hi != 0) || (before1 & lo != 0) {
                        self.v[0xF] = 1;
                    }
                }
                self.pc += 2;
            }
            Skp { .. }
            | Sknp { .. }
            | Lddv { .. }
            | Ldk { .. }
            | Ldvd { .. }
            | Ldsv { .. }
            | Addi { .. }
            | Ldfv { .. }
            | Ldbv { .. }
            | Ldiv { .. }
            | Ldvi { .. } => {}
            Unknown(_) => {}
        }
    }

    pub fn parseop(op: u16) -> ChipOp {
        match op & 0xF000 {
            0x0000 => match op {
                0x00E0 => ChipOp::Cls,
                0x00EE => ChipOp::Ret,
                _ => ChipOp::Unknown(op),
            },
            0x1000 => ChipOp::Jp { nnn: op & 0x0FFF },
            0x2000 => ChipOp::Call { nnn: op & 0x0FFF },
            0x3000 => ChipOp::Se {
                x: ((op & 0x0F00) >> 8) as u8,
                kk: (op & 0x00FF) as u8,
            },
            0x4000 => ChipOp::Sne {
                x: ((op & 0x0F00) >> 8) as u8,
                kk: (op & 0x00FF) as u8,
            },
            0x5000 => ChipOp::Ser {
                x: ((op & 0x0F00) >> 8) as u8,
                y: ((op & 0x00F0) >> 4) as u8,
            },
            0x6000 => ChipOp::Ld {
                x: ((op & 0x0F00) >> 8) as u8,
                kk: (op & 0x00FF) as u8,
            },
            0x7000 => ChipOp::Add {
                x: ((op & 0x0F00) >> 8) as u8,
                kk: (op & 0x00FF) as u8,
            },
            0x8000 => {
                let x = ((op & 0x0F00) >> 8) as u8;
                let y = ((op & 0x00F0) >> 4) as u8;
                match op & 0x000F {
                    0x0000 => ChipOp::Ldr { x, y },
                    0x0001 => ChipOp::Orr { x, y },
                    0x0002 => ChipOp::Andr { x, y },
                    0x0003 => ChipOp::Xorr { x, y },
                    0x0004 => ChipOp::Addr { x, y },
                    0x0005 => ChipOp::Subr { x, y },
                    0x0006 => ChipOp::Shrr { x, y },
                    0x0007 => ChipOp::Subnr { x, y },
                    0x000E => ChipOp::Shlr { x, y },
                    _ => ChipOp::Unknown(op),
                }
            }
            0x9000 => match op & 0x000F {
                0x0000 => ChipOp::Sner {
                    x: ((op & 0x0F00) >> 8) as u8,
                    y: ((op & 0x00F0) >> 4) as u8,
                },
                _ => ChipOp::Unknown(op),
            },
            0xA000 => ChipOp::Ldi { nnn: op & 0x0FFF },
            0xB000 => ChipOp::Jpo { nnn: op & 0x0FFF },
            0xC000 => ChipOp::Rnd {
                x: ((op & 0x0F00) >> 8) as u8,
                kk: (op & 0x00FF) as u8,
            },
            0xD000 => ChipOp::Drw {
                x: ((op & 0x0F00) >> 8) as u8,
                y: ((op & 0x00F0) >> 4) as u8,
                n: (op & 0x000F) as u8,
            },
            0xE000 => match op & 0x00FF {
                0x009E => ChipOp::Skp {
                    x: ((op & 0x0F00) >> 8) as u8,
                },
                0x00A1 => ChipOp::Sknp {
                    x: ((op & 0x0F00) >> 8) as u8,
                },
                _ => ChipOp::Unknown(op),
            },
            0xF000 => {
                let x = ((op & 0x0F00) >> 8) as u8;
                match op & 0x00FF {
                    0x0015 => ChipOp::Lddv { x },
                    0x0007 => ChipOp::Ldk { x },
                    0x000A => ChipOp::Ldvd { x },
                    0x0018 => ChipOp::Ldsv { x },
                    0x001E => ChipOp::Addi { x },
                    0x0029 => ChipOp::Ldfv { x },
                    0x0033 => ChipOp::Ldbv { x },
                    0x0055 => ChipOp::Ldiv { x },
                    0x0065 => ChipOp::Ldvi { x },
                    _ => ChipOp::Unknown(op),
                }
            }
            _ => ChipOp::Unknown(op),
        }
    }

    #[inline]
    fn vx(&mut self, x: u8) -> &mut u8 {
        &mut self.v[x as usize]
    }
}
