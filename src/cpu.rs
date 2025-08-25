use ndarray::Array2;
use random_number::random;

use crate::mem::Memory;
use crate::op::ChipOp;
use crate::{consts::PROGRAM_START, decode::decode};
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};

use crate::consts::{CHIP8_FONTSET, H, W};

#[derive(Default, Copy, Clone)]
pub enum Resolution {
    #[default]
    Low,
    High,
}

impl Resolution {
    pub fn factor(&self) -> usize {
        match self {
            Resolution::High => 2,
            Resolution::Low => 1,
        }
    }
}

#[derive(Default, Clone)]
pub enum KeyState {
    #[default]
    AwaitingPress,
    AwaitingRelease,
}

pub type Screen = Array2<u8>;

#[derive(Default, Clone)]
pub struct Chip8 {
    pub pc: usize,         // Program counter
    pub v: [u8; 16],       // General purpose registers
    pub i: usize,          // Address register
    pub sp: usize,         // Stack Pointer
    pub dt: Arc<AtomicU8>, // Delay timer
    pub st: Arc<AtomicU8>, // Sound timer
    pub keys: [bool; 16],
    pub stack: [usize; 16],
    pub screen: Screen,
    pub memory: Memory,
    pub resolution: Resolution,
    pub key_state: KeyState,
    pub last_key: u8,
    pub exit: bool,
}

impl Chip8 {
    pub fn new() -> Self {
        Chip8 {
            pc: PROGRAM_START,
            screen: Array2::<u8>::zeros((H, W)),
            ..Chip8::default()
        }
    }
    pub fn load_font(&mut self) {
        let base = 0x0;
        self.memory[base..base + CHIP8_FONTSET.len()].copy_from_slice(&CHIP8_FONTSET);
    }
    pub fn press_key(&mut self, key: u8) {
        self.keys[key as usize] = true;
    }
    pub fn release_key(&mut self, key: u8) {
        self.keys[key as usize] = false;
    }
    pub fn run_step(&mut self, cycles: u64) {
        for _ in 0..cycles {
            let b = self.memory[self.pc];
            let s = self.memory[self.pc + 1];
            let op = decode(u16::from_be_bytes([b, s]));
            self.exec(op);
        }
    }
    pub fn exec(&mut self, op: ChipOp) {
        use ChipOp::*;
        match op {
            ScdN { n } => {
                let screen = self.screen.clone();
                for (y, mut row) in self.screen.outer_iter_mut().enumerate() {
                    for (x, elem) in row.iter_mut().enumerate() {
                        let y_shifted: i16 = (y as i16) - (n as i16);
                        if y_shifted >= 0 {
                            *elem = screen[(y_shifted as usize, x)]
                        } else {
                            *elem = 0;
                        }
                    }
                }
                self.pc += 2;
            }
            ScuN { n } => {
                let screen = self.screen.clone();
                let (nrows, _) = self.screen.dim();
                for (y, mut row) in self.screen.outer_iter_mut().enumerate() {
                    for (x, elem) in row.iter_mut().enumerate() {
                        let y_shifted: usize = y + (n as usize);
                        if y_shifted < nrows {
                            *elem = screen[(y_shifted, x)]
                        } else {
                            *elem = 0;
                        }
                    }
                }
                self.pc += 2;
            }
            Cls => {
                self.screen.fill(0);
                self.pc += 2;
            }
            Ret => {
                self.pc = self.stack[self.sp - 1];
                self.sp -= 1;
            }

            Scr => {
                let screen = self.screen.clone();
                for (y, mut row) in self.screen.outer_iter_mut().enumerate() {
                    for (x, elem) in row.iter_mut().enumerate() {
                        let mut tmp = screen[(y, x)] >> 4;
                        if x > 0 {
                            tmp |= screen[(y, x - 1)] << 4;
                        }
                        *elem = tmp
                    }
                }
                self.pc += 2;
            }
            Scl => {
                let screen = self.screen.clone();
                let (_, ncols) = self.screen.dim();
                for (y, mut row) in self.screen.outer_iter_mut().enumerate() {
                    for (x, elem) in row.iter_mut().enumerate() {
                        let mut tmp = screen[(y, x)] << 4;
                        if x < ncols - 1 {
                            tmp |= screen[(y, x + 1)] >> 4;
                        }
                        *elem = tmp
                    }
                }
                self.pc += 2;
            }
            Exit => {
                self.exit = true;
            }
            LowRes => {
                self.resolution = Resolution::Low;
                self.pc += 2;
            }
            HighRes => {
                self.resolution = Resolution::High;
                self.pc += 2;
            }
            JpNnn { nnn } => {
                self.pc = nnn;
            }
            CallNnn { nnn } => {
                self.sp += 1;
                self.stack[self.sp - 1] = self.pc + 2;
                self.pc = nnn;
            }
            SeVxNn { x, nn } => {
                if self.v[x] == nn {
                    self.pc += 4;
                } else {
                    self.pc += 2;
                }
            }
            SneVxNn { x, nn } => {
                if *self.vx(x) != nn {
                    self.pc += 4;
                } else {
                    self.pc += 2;
                }
            }
            SeVxVy { x, y } => {
                if *self.vx(x) == *self.vx(y) {
                    self.pc += 4;
                } else {
                    self.pc += 2;
                }
            }
            LdVxVyI { x, y } => {
                if y < x {
                    panic!("LdVxVyI: VY must be a higher register than VX");
                }
                let range_end = self.i + y - x;
                let mem_range = self.i..=range_end;
                self.memory[mem_range].copy_from_slice(&self.v[x..=y]);
            }
            LdIVxVy { x, y } => {
                if y < x {
                    panic!("LdVxVyI: VY must be a higher register than VX");
                }
                let range_end = self.i + y - x;
                let mem_range = self.i..=range_end;
                self.v[x..=y].copy_from_slice(&self.memory[mem_range]);
            }
            LdVxNn { x, nn } => {
                *self.vx(x) = nn;
                self.pc += 2;
            }
            AddVxNn { x, nn } => {
                let r = self.vx(x);
                *r = r.wrapping_add(nn);
                self.pc += 2;
            }
            LdVxVy { x, y } => {
                let vy = *self.vx(y);
                let vx = self.vx(x);
                *vx = vy;
                self.pc += 2;
            }
            OrVxVy { x, y } => {
                let vy = *self.vx(y);
                let vx = self.vx(x);
                *vx |= vy;
                self.pc += 2;
            }
            AndVxVy { x, y } => {
                let vy = *self.vx(y);
                let vx = self.vx(x);
                *vx &= vy;
                self.pc += 2;
            }
            XorVxVy { x, y } => {
                let vy = *self.vx(y);
                let vx = self.vx(x);
                *vx ^= vy;
                self.pc += 2;
            }
            AddVxVy { x, y } => {
                let vy = *self.vx(y);
                let vx = *self.vx(x);
                let (res, carry) = vx.overflowing_add(vy);
                *self.vx(x) = res;
                self.v[0xF] = carry as u8;
                self.pc += 2;
            }
            SubVxVy { x, y } => {
                let vy = *self.vx(y);
                let vx = *self.vx(x);
                let (res, overflow) = vx.overflowing_sub(vy);
                *self.vx(x) = res;
                self.v[0xF] = !overflow as u8;
                self.pc += 2;
            }
            ShrVxVy { x, y } => {
                let vy = *self.vx(y);
                *self.vx(x) = vy >> 1;
                self.v[0xF] = vy & 0x1;
                self.pc += 2;
            }
            SubnVxVy { x, y } => {
                let vy = *self.vx(y);
                let vx = *self.vx(x);
                let (res, overflow) = vy.overflowing_sub(vx);
                *self.vx(x) = res;
                self.v[0xF] = !overflow as u8;
                self.pc += 2;
            }
            ShlVxVy { x, y } => {
                let vy = *self.vx(y);
                *self.vx(x) = vy << 1;
                self.v[0xF] = vy >> 7;
                self.pc += 2;
            }
            SneVxVy { x, y } => {
                if *self.vx(x) != *self.vx(y) {
                    self.pc += 4;
                } else {
                    self.pc += 2;
                }
            }
            LdINnn { nnn } => {
                self.i = nnn;
                self.pc += 2;
            }
            JpV0Nnn { nnn } => {
                self.pc = (nnn + (*self.vx(0) as u16)) as usize;
            }
            RndVxNn { x, nn } => {
                let n: u8 = random!();
                *self.vx(x) = n & nn;
                self.pc += 2;
            }
            DrwVxVyN { x, y, n } => {
                let vx = *self.vx(x) as usize;
                let vy = *self.vx(y) as usize;
                let bit_off = vx & 7; // vx % 8
                let col_byte = vx >> 3; // vx / 8
                let height = n as usize;

                let (rows, bytes_per_row) = self.screen.dim();

                // collision flag (VF)
                self.v[0xF] = 0;

                for (row, &byte) in self.memory[self.i..self.i + height].iter().enumerate() {
                    let y_idx = (vy + row) % rows;
                    let x0 = col_byte % bytes_per_row;
                    let x1 = (col_byte + 1) % bytes_per_row; // next byte (wrap horizontally)

                    // Shift the 8-bit sprite line by bit_off across two bytes.
                    let shifted = (u16::from(byte) << 8) >> bit_off;
                    let [hi, lo] = shifted.to_be_bytes();

                    // Cache low and hi bytes to check collision flag
                    let before0 = self.screen[(y_idx, x0)];
                    let before1 = self.screen[(y_idx, x1)];

                    self.screen[(y_idx, x0)] ^= hi;
                    self.screen[(y_idx, x1)] ^= lo;

                    // Check and set collision flag (VF)
                    if (before0 & hi != 0) || (before1 & lo != 0) {
                        self.v[0xF] = 1;
                    }
                }
                self.pc += 2;
            }
            SkpVx { x } => {
                let vx = *self.vx(x);
                if self.keys[(vx & 0xF) as usize] {
                    self.pc += 4
                } else {
                    self.pc += 2
                }
            }
            SknpVx { x } => {
                let vx = *self.vx(x);
                if !self.keys[(vx & 0xF) as usize] {
                    self.pc += 4
                } else {
                    self.pc += 2
                }
            }
            LdDtVx { x } => {
                let val = *self.vx(x);
                self.dt.store(val, Ordering::Release);
                self.pc += 2;
            }
            LdVxDt { x } => {
                *self.vx(x) = self.dt.load(Ordering::Acquire);
                self.pc += 2;
            }
            LdVxK { x } => match self.key_state {
                KeyState::AwaitingPress => {
                    for (key, pressed) in self.keys.into_iter().enumerate() {
                        if pressed {
                            self.key_state = KeyState::AwaitingRelease;
                            self.last_key = key as u8;
                            break;
                        }
                    }
                }
                KeyState::AwaitingRelease => {
                    let all_clear = self.keys.iter().all(|&k| !k);
                    if all_clear {
                        self.key_state = KeyState::AwaitingPress;
                        *self.vx(x) = self.last_key;
                        self.pc += 2;
                    }
                }
            },
            LdStVx { x } => {
                let val = *self.vx(x);
                self.st.store(val, Ordering::Release);
                self.pc += 2;
            }
            AddIVx { x } => {
                let vx = *self.vx(x);
                self.i += vx as usize;
                self.pc += 2;
            }
            LdFVx { x } => {
                // set I to the 5 line high hex sprite for the lowest nibble in vX
                let vx = *self.vx(x) & 0x0F;
                self.i = (vx * 5) as usize;
                self.pc += 2;
            }
            LdBVx { x } => {
                let vx = *self.vx(x);
                self.memory[self.i] = (vx % 255) / 100;
                self.memory[self.i + 1] = (vx % 100) / 10;
                self.memory[self.i + 2] = vx % 10;
                self.pc += 2;
            }
            LdIVx { x } => {
                for vx in &mut self.v[0..=x] {
                    self.memory[self.i] = *vx;
                    self.i += 1;
                }
                self.pc += 2;
            }
            LdVxI { x } => {
                for vx in &mut self.v[0..=x] {
                    *vx = self.memory[self.i];
                    self.i += 1;
                }
                self.pc += 2;
            }
            Unknown(x) => {
                panic!("Unkown opcode: {x:#05X}");
            }
        }
    }

    #[inline]
    fn vx(&mut self, x: usize) -> &mut u8 {
        &mut self.v[x]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_ret() {
        let pc = 0x200;
        let mut chip: Chip8 = Default::default();
        chip.stack[0] = pc;
        chip.sp = 1;
        chip.pc = 0xABC;

        chip.exec(ChipOp::Ret);
        assert!(chip.sp == 0);
        assert!(chip.pc == pc);
    }

    #[test]
    fn test_exec_jp() {
        let pc = 0x400;
        let mut chip: Chip8 = Default::default();
        let op = ChipOp::JpNnn { nnn: pc };
        chip.exec(op);
        assert!(chip.pc == pc);
    }

    #[test]
    fn test_exec_call() {
        let addr = 0xABC;
        let mut chip = Chip8::new();

        chip.exec(ChipOp::CallNnn { nnn: addr });
        assert!(chip.sp == 1);
        assert!(chip.pc == addr);
    }

    #[test]
    fn test_exec_se_skip() {
        let mut chip = Chip8::new();
        chip.v[0] = 20;

        chip.exec(ChipOp::SeVxNn { x: 0, nn: 20 });
        assert!(chip.pc == 0x204);
    }

    #[test]
    fn test_exec_se_no_skip() {
        let mut chip = Chip8::new();
        chip.v[1] = 10;

        chip.exec(ChipOp::SeVxNn { x: 1, nn: 20 });
        assert!(chip.pc == 0x202);
    }

    #[test]
    fn test_exec_sne_no_skip() {
        let mut chip = Chip8::new();
        chip.v[0] = 20;

        chip.exec(ChipOp::SneVxNn { x: 0, nn: 20 });
        assert!(chip.pc == 0x202);
    }

    #[test]
    fn test_exec_sne_skip() {
        let mut chip = Chip8::new();
        chip.v[1] = 10;

        chip.exec(ChipOp::SneVxNn { x: 1, nn: 20 });
        assert!(chip.pc == 0x204);
    }

    #[test]
    fn test_exec_ser_skip() {
        let mut chip = Chip8::new();
        chip.v[0] = 20;
        chip.v[1] = 20;

        chip.exec(ChipOp::SeVxVy { x: 0, y: 1 });
        assert!(chip.pc == 0x204);
    }

    #[test]
    fn test_exec_ser_no_skip() {
        let mut chip = Chip8::new();
        chip.v[0] = 20;
        chip.v[1] = 17;

        chip.exec(ChipOp::SeVxVy { x: 0, y: 1 });
        assert!(chip.pc == 0x202);
    }

    #[test]
    fn test_exec_ld_vx_vy_i() {
        let mut chip = Chip8::new();
        chip.v[0] = 20;
        chip.v[1] = 17;
        chip.v[2] = 12;
        chip.v[3] = 42;
        chip.v[4] = 0xBF;
        chip.i = 0x400;

        chip.exec(ChipOp::LdVxVyI { x: 0, y: 3 });
        assert!(chip.memory[0x400] == 20);
        assert!(chip.memory[0x401] == 17);
        assert!(chip.memory[0x402] == 12);
        assert!(chip.memory[0x403] == 42);
        assert!(chip.memory[0x404] != 0xBF);
    }

    #[test]
    fn test_exec_ld_i_vx_vy() {
        let mut chip = Chip8::new();
        chip.memory[0x400] = 20;
        chip.memory[0x401] = 17;
        chip.memory[0x402] = 12;
        chip.memory[0x403] = 42;
        chip.memory[0x404] = 0xBF;
        chip.i = 0x400;

        chip.exec(ChipOp::LdIVxVy { x: 0, y: 3 });

        assert!(chip.v[0] == 20);
        assert!(chip.v[1] == 17);
        assert!(chip.v[2] == 12);
        assert!(chip.v[3] == 42);
        assert!(chip.v[4] != 0xBF);
    }

    #[test]
    fn test_exec_ld() {
        let reg = 3;
        let mut chip = Chip8::new();

        chip.exec(ChipOp::LdVxNn { x: reg, nn: 0xAB });
        assert_eq!(chip.pc, 0x202);
        assert!(chip.v[reg] == 0xAB);
    }

    #[test]
    fn test_exec_add() {
        let reg = 3;
        let mut chip = Chip8::new();

        chip.exec(ChipOp::AddVxNn { x: reg, nn: 0xA0 });
        assert_eq!(chip.pc, 0x202);
        assert!(chip.v[reg] == 0xA0);

        chip.exec(ChipOp::AddVxNn { x: reg, nn: 0x0B });
        assert_eq!(chip.pc, 0x204);
        assert!(chip.v[reg] == 0xAB);
    }

    #[test]
    fn test_exec_ldr() {
        let x = 3;
        let y = 5;
        let mut chip = Chip8::new();
        chip.v[y] = 0xAB;

        chip.exec(ChipOp::LdVxVy { x, y });
        assert_eq!(chip.pc, 0x202);
        assert!(chip.v[x] == 0xAB);
    }

    #[test]
    fn test_run_drw_row() {
        let img_loc = 0x400;
        let mut chip = Chip8::new();
        chip.v[0] = 0;
        chip.v[1] = 0;
        chip.i = img_loc;
        chip.memory[img_loc] = 0xAB;
        chip.exec(ChipOp::DrwVxVyN { x: 0, y: 1, n: 1 });
        assert_eq!(chip.pc, 0x202);
        assert!(chip.screen[(0, 0)] == 0xAB);
    }

    #[test]
    fn test_run_drw_row_x_offset() {
        let img_loc = 0x400;
        let mut chip = Chip8::new();
        chip.v[0] = 1;
        chip.v[1] = 0;
        chip.i = img_loc;
        chip.memory[img_loc] = 0b11110000;
        chip.exec(ChipOp::DrwVxVyN { x: 0, y: 1, n: 1 });
        assert_eq!(chip.pc, 0x202);
        assert!(chip.screen[(0, 0)] == 0b01111000);
    }

    #[test]
    fn test_run_drw_row_x_offset_byte_boundary() {
        let img_loc = 0x400;
        let mut chip = Chip8::new();
        chip.v[0] = 6;
        chip.v[1] = 0;
        chip.i = img_loc;
        chip.memory[img_loc] = 0b11110000;
        chip.exec(ChipOp::DrwVxVyN { x: 0, y: 1, n: 1 });
        assert_eq!(chip.pc, 0x202);
        assert!(chip.screen[(0, 0)] == 0b00000011);
        assert!(chip.screen[(0, 1)] == 0b11000000);
    }

    #[test]
    fn test_run_drw_row_x_offset_big() {
        let img_loc = 0x400;
        let mut chip = Chip8::new();
        chip.v[0] = 13;
        chip.v[1] = 0;
        chip.i = img_loc;
        chip.memory[img_loc] = 0b11110000;
        chip.exec(ChipOp::DrwVxVyN { x: 0, y: 1, n: 1 });
        assert_eq!(chip.pc, 0x202);
        assert!(chip.screen[(0, 1)] == 0b00000111);
        assert!(chip.screen[(0, 2)] == 0b10000000);
    }

    #[test]
    fn test_run_drw_zero() {
        let img_loc: usize = 0x400;
        let mut chip = Chip8::new();
        chip.v[0] = 0;
        chip.v[1] = 0;
        chip.i = img_loc;
        chip.memory[img_loc] = 0xF0;
        chip.memory[img_loc + 1] = 0x90;
        chip.memory[img_loc + 2] = 0x90;
        chip.memory[img_loc + 3] = 0x90;
        chip.memory[img_loc + 4] = 0xF0;
        chip.exec(ChipOp::DrwVxVyN { x: 0, y: 1, n: 5 });
        assert_eq!(chip.pc, 0x202);
        assert!(chip.screen[(0, 0)] == 0xF0);
        assert!(chip.screen[(1, 0)] == 0x90);
        assert!(chip.screen[(2, 0)] == 0x90);
        assert!(chip.screen[(3, 0)] == 0x90);
        assert!(chip.screen[(4, 0)] == 0xF0);
    }

    #[test]
    fn test_run_drw_zero_y_offset() {
        let img_loc: usize = 0x400;
        let mut chip = Chip8::new();
        chip.v[0] = 0;
        chip.v[1] = 1;
        chip.i = img_loc;
        chip.memory[img_loc] = 0xF0;
        chip.memory[img_loc + 1] = 0x90;
        chip.memory[img_loc + 2] = 0x90;
        chip.memory[img_loc + 3] = 0x90;
        chip.memory[img_loc + 4] = 0xF0;
        chip.exec(ChipOp::DrwVxVyN { x: 0, y: 1, n: 5 });
        assert_eq!(chip.pc, 0x202);
        assert!(chip.screen[(1, 0)] == 0xF0);
        assert!(chip.screen[(2, 0)] == 0x90);
        assert!(chip.screen[(3, 0)] == 0x90);
        assert!(chip.screen[(4, 0)] == 0x90);
        assert!(chip.screen[(5, 0)] == 0xF0);
    }

    #[test]
    fn test_run_drw_zero_xy_offset() {
        let img_loc: usize = 0x400;
        let mut chip = Chip8::new();
        chip.v[0] = 4;
        chip.v[1] = 1;
        chip.i = img_loc;
        chip.memory[img_loc] = 0xF0;
        chip.memory[img_loc + 1] = 0x90;
        chip.memory[img_loc + 2] = 0x90;
        chip.memory[img_loc + 3] = 0x90;
        chip.memory[img_loc + 4] = 0xF0;
        chip.exec(ChipOp::DrwVxVyN { x: 0, y: 1, n: 5 });
        assert_eq!(chip.pc, 0x202);
        assert!(chip.screen[(1, 0)] == 0x0F);
        assert!(chip.screen[(2, 0)] == 0x09);
        assert!(chip.screen[(3, 0)] == 0x09);
        assert!(chip.screen[(4, 0)] == 0x09);
        assert!(chip.screen[(5, 0)] == 0x0F);
    }

    #[test]
    fn test_run_drw_test_collision() {
        let img_loc: usize = 0x400;
        let mut chip = Chip8::new();
        chip.v[0] = 4;
        chip.v[1] = 1;
        chip.i = img_loc;
        chip.memory[img_loc] = 0xF0;
        chip.memory[img_loc + 1] = 0x90;
        chip.memory[img_loc + 2] = 0x90;
        chip.memory[img_loc + 3] = 0x90;
        chip.memory[img_loc + 4] = 0xF0;

        // Test first drw has no collision
        chip.exec(ChipOp::DrwVxVyN { x: 0, y: 1, n: 5 });
        assert!(chip.v[0xF] == 0);
        assert_eq!(chip.pc, 0x202);

        // Change offset and check that the collision flag is set
        chip.exec(ChipOp::DrwVxVyN { x: 0, y: 1, n: 4 });
        assert!(chip.v[0x1] == 1);
        assert_eq!(chip.pc, 0x204);
    }

    #[test]
    fn test_exec_clr() {
        let mut chip = Chip8::new();

        chip.screen[(0, 0)] = 0xFF;
        chip.screen[(10, 5)] = 0x0F;
        chip.v[0xF] = 1;

        chip.exec(ChipOp::Cls);
        assert_eq!(chip.pc, 0x202);

        assert_eq!(chip.screen.iter().sum::<u8>(), 0);
    }

    #[test]
    fn test_exec_or_vx_vy() {
        let mut chip = Chip8::new();
        chip.v[0] = 0b10101010;
        chip.v[1] = 0b01010101;

        chip.exec(ChipOp::OrVxVy { x: 0, y: 1 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.v[0], 0b11111111);
    }

    #[test]
    fn test_exec_and_vx_vy() {
        let mut chip = Chip8::new();
        chip.v[0] = 0b11110000;
        chip.v[1] = 0b10101010;

        chip.exec(ChipOp::AndVxVy { x: 0, y: 1 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.v[0], 0b10100000);
    }

    #[test]
    fn test_exec_xor_vx_vy() {
        let mut chip = Chip8::new();
        chip.v[0] = 0b11110000;
        chip.v[1] = 0b10101010;

        chip.exec(ChipOp::XorVxVy { x: 0, y: 1 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.v[0], 0b01011010);
    }

    #[test]
    fn test_exec_add_vx_vy_no_carry() {
        let mut chip = Chip8::new();
        chip.v[0] = 50;
        chip.v[1] = 100;

        chip.exec(ChipOp::AddVxVy { x: 0, y: 1 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.v[0], 150);
        assert_eq!(chip.v[0xF], 0);
    }

    #[test]
    fn test_exec_add_vx_vy_with_carry() {
        let mut chip = Chip8::new();
        chip.v[0] = 200;
        chip.v[1] = 100;

        chip.exec(ChipOp::AddVxVy { x: 0, y: 1 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.v[0], 44); // 300 & 0xFF
        assert_eq!(chip.v[0xF], 1);
    }

    #[test]
    fn test_exec_sub_vx_vy_no_borrow() {
        let mut chip = Chip8::new();
        chip.v[0] = 100;
        chip.v[1] = 50;

        chip.exec(ChipOp::SubVxVy { x: 0, y: 1 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.v[0], 50);
        assert_eq!(chip.v[0xF], 1);
    }

    #[test]
    fn test_exec_sub_vx_vy_with_borrow() {
        let mut chip = Chip8::new();
        chip.v[0] = 50;
        chip.v[1] = 100;

        chip.exec(ChipOp::SubVxVy { x: 0, y: 1 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.v[0], 206); // wrapping sub
        assert_eq!(chip.v[0xF], 0);
    }

    #[test]
    fn test_exec_shr_vx_vy() {
        let mut chip = Chip8::new();
        chip.v[1] = 0b10101011;

        chip.exec(ChipOp::ShrVxVy { x: 0, y: 1 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.v[0], 0b01010101);
        assert_eq!(chip.v[0xF], 1);
    }

    #[test]
    fn test_exec_subn_vx_vy_no_borrow() {
        let mut chip = Chip8::new();
        chip.v[0] = 50;
        chip.v[1] = 100;

        chip.exec(ChipOp::SubnVxVy { x: 0, y: 1 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.v[0], 50);
        assert_eq!(chip.v[0xF], 1);
    }

    #[test]
    fn test_exec_subn_vx_vy_with_borrow() {
        let mut chip = Chip8::new();
        chip.v[0] = 100;
        chip.v[1] = 50;

        chip.exec(ChipOp::SubnVxVy { x: 0, y: 1 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.v[0], 206); // wrapping sub
        assert_eq!(chip.v[0xF], 0);
    }

    #[test]
    fn test_exec_shl_vx_vy() {
        let mut chip = Chip8::new();
        chip.v[1] = 0b10101011;

        chip.exec(ChipOp::ShlVxVy { x: 0, y: 1 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.v[0], 0b01010110);
        assert_eq!(chip.v[0xF], 1);
    }

    #[test]
    fn test_exec_sne_vx_vy_skip() {
        let mut chip = Chip8::new();
        chip.v[0] = 20;
        chip.v[1] = 30;

        chip.exec(ChipOp::SneVxVy { x: 0, y: 1 });
        assert_eq!(chip.pc, 0x204);
    }

    #[test]
    fn test_exec_sne_vx_vy_no_skip() {
        let mut chip = Chip8::new();
        chip.v[0] = 20;
        chip.v[1] = 20;

        chip.exec(ChipOp::SneVxVy { x: 0, y: 1 });
        assert_eq!(chip.pc, 0x202);
    }

    #[test]
    fn test_exec_ld_i_nnn() {
        let mut chip = Chip8::new();

        chip.exec(ChipOp::LdINnn { nnn: 0x400 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.i, 0x400);
    }

    #[test]
    fn test_exec_jp_v0_nnn() {
        let mut chip = Chip8::new();
        chip.v[0] = 0x10;

        chip.exec(ChipOp::JpV0Nnn { nnn: 0x300 });
        assert_eq!(chip.pc, 0x310);
    }

    #[test]
    fn test_exec_rnd_vx_nn() {
        let mut chip = Chip8::new();

        chip.exec(ChipOp::RndVxNn { x: 0, nn: 0x0F });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.v[0] & 0xF0, 0);
    }

    #[test]
    fn test_exec_skp_vx_pressed() {
        let mut chip = Chip8::new();
        chip.v[0] = 5;
        chip.keys[5] = true;

        chip.exec(ChipOp::SkpVx { x: 0 });
        assert_eq!(chip.pc, 0x204);
    }

    #[test]
    fn test_exec_skp_vx_not_pressed() {
        let mut chip = Chip8::new();
        chip.v[0] = 5;
        chip.keys[5] = false;

        chip.exec(ChipOp::SkpVx { x: 0 });
        assert_eq!(chip.pc, 0x202);
    }

    #[test]
    fn test_exec_sknp_vx_not_pressed() {
        let mut chip = Chip8::new();
        chip.v[0] = 5;
        chip.keys[5] = false;

        chip.exec(ChipOp::SknpVx { x: 0 });
        assert_eq!(chip.pc, 0x204);
    }

    #[test]
    fn test_exec_sknp_vx_pressed() {
        let mut chip = Chip8::new();
        chip.v[0] = 5;
        chip.keys[5] = true;

        chip.exec(ChipOp::SknpVx { x: 0 });
        assert_eq!(chip.pc, 0x202);
    }

    #[test]
    fn test_exec_ld_vx_dt() {
        let mut chip = Chip8::new();
        chip.dt.store(42, Ordering::Release);

        chip.exec(ChipOp::LdVxDt { x: 0 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.v[0], 42);
    }

    #[test]
    fn test_exec_ld_dt_vx() {
        let mut chip = Chip8::new();
        chip.v[0] = 42;

        chip.exec(ChipOp::LdDtVx { x: 0 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.dt.load(Ordering::Acquire), 42);
    }

    #[test]
    fn test_exec_ld_st_vx() {
        let mut chip = Chip8::new();
        chip.v[0] = 42;

        chip.exec(ChipOp::LdStVx { x: 0 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.st.load(Ordering::Acquire), 42);
    }

    #[test]
    fn test_exec_add_i_vx() {
        let mut chip = Chip8::new();
        chip.i = 0x300;
        chip.v[0] = 0x10;

        chip.exec(ChipOp::AddIVx { x: 0 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.i, 0x310);
    }

    #[test]
    fn test_exec_ld_f_vx() {
        let mut chip = Chip8::new();
        chip.v[0] = 0xA;

        chip.exec(ChipOp::LdFVx { x: 0 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.i, 50); // 0xA * 5
    }

    #[test]
    fn test_exec_ld_b_vx() {
        let mut chip = Chip8::new();
        chip.v[0] = 123;
        chip.i = 0x300;

        chip.exec(ChipOp::LdBVx { x: 0 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.memory[0x300], 1);
        assert_eq!(chip.memory[0x301], 2);
        assert_eq!(chip.memory[0x302], 3);
    }

    #[test]
    fn test_exec_ld_i_vx() {
        let mut chip = Chip8::new();
        chip.v[0] = 0xAB;
        chip.v[1] = 0xCD;
        chip.v[2] = 0xEF;
        chip.i = 0x300;

        chip.exec(ChipOp::LdIVx { x: 2 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.memory[0x300], 0xAB);
        assert_eq!(chip.memory[0x301], 0xCD);
        assert_eq!(chip.memory[0x302], 0xEF);
        assert_eq!(chip.i, 0x303);
    }

    #[test]
    fn test_exec_ld_vx_i() {
        let mut chip = Chip8::new();
        chip.i = 0x300;
        chip.memory[0x300] = 0xAB;
        chip.memory[0x301] = 0xCD;
        chip.memory[0x302] = 0xEF;

        chip.exec(ChipOp::LdVxI { x: 2 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.v[0], 0xAB);
        assert_eq!(chip.v[1], 0xCD);
        assert_eq!(chip.v[2], 0xEF);
        assert_eq!(chip.i, 0x303);
    }

    #[test]
    fn test_exec_scd_n() {
        let mut chip = Chip8::new();
        chip.screen[(0, 0)] = 0xFF;
        chip.screen[(1, 0)] = 0xAA;
        chip.screen[(2, 0)] = 0x55;

        chip.exec(ChipOp::ScdN { n: 1 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.screen[(0, 0)], 0);
        assert_eq!(chip.screen[(1, 0)], 0xFF);
        assert_eq!(chip.screen[(2, 0)], 0xAA);
    }

    #[test]
    fn test_exec_scu_n() {
        let mut chip = Chip8::new();
        chip.screen[(0, 0)] = 0xFF;
        chip.screen[(1, 0)] = 0xAA;
        chip.screen[(2, 0)] = 0x55;

        chip.exec(ChipOp::ScuN { n: 1 });
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.screen[(0, 0)], 0xAA);
        assert_eq!(chip.screen[(1, 0)], 0x55);
        assert_eq!(chip.screen[(2, 0)], 0);
    }

    #[test]
    fn test_exec_scr() {
        let mut chip = Chip8::new();
        chip.screen[(0, 0)] = 0b11110000;
        chip.screen[(0, 1)] = 0b10101010;

        chip.exec(ChipOp::Scr);
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.screen[(0, 0)], 0b00001111);
        assert_eq!(chip.screen[(0, 1)], 0b00001010);
    }

    #[test]
    fn test_exec_scl() {
        let mut chip = Chip8::new();
        chip.screen[(0, 0)] = 0b11110000;
        chip.screen[(0, 1)] = 0b10101010;

        chip.exec(ChipOp::Scl);
        assert_eq!(chip.pc, 0x202);
        assert_eq!(chip.screen[(0, 0)], 0b00001010);
        assert_eq!(chip.screen[(0, 1)], 0b10100000);
    }

    #[test]
    fn test_exec_exit() {
        let mut chip = Chip8::new();
        assert!(!chip.exit);

        chip.exec(ChipOp::Exit);
        assert!(chip.exit);
    }

    #[test]
    fn test_exec_low_res() {
        let mut chip = Chip8::new();
        chip.resolution = Resolution::High;

        chip.exec(ChipOp::LowRes);
        assert_eq!(chip.pc, 0x202);
        assert!(matches!(chip.resolution, Resolution::Low));
    }

    #[test]
    fn test_exec_high_res() {
        let mut chip = Chip8::new();
        chip.resolution = Resolution::Low;

        chip.exec(ChipOp::HighRes);
        assert_eq!(chip.pc, 0x202);
        assert!(matches!(chip.resolution, Resolution::High));
    }

    #[test]
    fn test_exec_ld_vx_k_awaiting_press() {
        let mut chip = Chip8::new();
        chip.key_state = KeyState::AwaitingPress;
        chip.keys[5] = true;

        chip.exec(ChipOp::LdVxK { x: 0 });
        assert_eq!(chip.pc, 0x200); // PC not incremented yet
        assert!(matches!(chip.key_state, KeyState::AwaitingRelease));
        assert_eq!(chip.last_key, 5);
    }

    #[test]
    fn test_exec_ld_vx_k_awaiting_release() {
        let mut chip = Chip8::new();
        chip.key_state = KeyState::AwaitingRelease;
        chip.last_key = 5;
        chip.keys.fill(false); // All keys released

        chip.exec(ChipOp::LdVxK { x: 0 });
        assert_eq!(chip.pc, 0x202);
        assert!(matches!(chip.key_state, KeyState::AwaitingPress));
        assert_eq!(chip.v[0], 5);
    }
}
