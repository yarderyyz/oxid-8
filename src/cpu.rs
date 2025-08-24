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

#[derive(Default, Clone)]
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
    pub fn run_step(&mut self) {
        for _ in 0..8 {
            let b = self.memory[self.pc];
            let s = self.memory[self.pc + 1];
            let op = decode(u16::from_be_bytes([b, s]));
            self.exec(op);
        }
    }
    pub fn exec(&mut self, op: ChipOp) {
        use ChipOp::*;
        match op {
            Cls => {
                self.screen.fill(0);
                self.pc += 2;
            }
            Ret => {
                self.pc = self.stack[self.sp - 1];
                self.sp -= 1;
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
            ShrVxVy { x, .. } => {
                let vx = *self.vx(x);
                *self.vx(x) = vx >> 1;
                self.v[0xF] = vx & 0x1;
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
            ShlVxVy { x, .. } => {
                let vx = *self.vx(x);
                *self.vx(x) = vx << 1;
                self.v[0xF] = vx >> 7;
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
        let pc = 0x200;
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
}
