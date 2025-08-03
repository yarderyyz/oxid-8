use cpal::traits::StreamTrait;

use random_number::random;

use std::sync::{atomic::AtomicU8, atomic::Ordering, Arc};
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use crate::audio;
use crate::mem::Memory;
use crate::op::ChipOp;

use crate::consts::W;
use crate::consts::{CHIP8_FONTSET, H};

#[derive(Default)]
pub enum KeyState {
    #[default]
    AwaitingPress,
    AwaitingRelease,
}

#[derive(Default)]
pub struct Chip8 {
    pub pc: usize,         // Program counter
    pub v: [u8; 16],       // General purpose registers
    pub i: usize,          // Address register
    pub sp: usize,         // Stack Pointer
    pub dt: Arc<AtomicU8>, // Delay timer
    pub st: Arc<AtomicU8>, // Sound timer
    pub keys: [bool; 16],
    pub stack: [usize; 16],
    pub screen: [[u8; W]; H],
    pub memory: Memory,
    pub key_state: KeyState,
    pub last_key: u8,
}

impl Chip8 {
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
            let op = Chip8::parseop(u16::from_be_bytes([b, s]));
            self.run_op(op);
        }
    }
    pub fn start_counters(&self) {
        let dt = self.dt.clone();
        let st = self.dt.clone();

        let mut sounding = false;
        thread::spawn(move || {
            let stream = audio::setup().unwrap();
            loop {
                let _ = dt.fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                    if v > 0 {
                        Some(v - 1)
                    } else {
                        None
                    }
                });
                let _ = st.fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                    if v > 0 {
                        Some(v - 1)
                    } else {
                        None
                    }
                });

                let st_now = st.load(Ordering::Acquire);
                if st_now > 0 && !sounding {
                    let _ = stream.play();
                    print!("sounding");
                    sounding = true;
                } else if st_now == 0 && sounding {
                    let _ = stream.pause();
                    sounding = false;
                }
                sleep(Duration::from_secs_f64(1.0 / 60.0));
            }
        });
    }

    // pub fn run<F: Fn(&Chip8)>(&mut self, render: F) {
    //     loop {
    //         self.run_step();
    //         render(self);
    //     }
    // }

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
                self.stack[self.sp - 1] = self.pc + 2;
                self.pc = nnn;
            }
            Se { x, kk } => {
                if self.v[x] == kk {
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
            Orr { x, y } => {
                let vy = *self.vx(y);
                let vx = self.vx(x);
                *vx |= vy;
                self.pc += 2;
            }
            Andr { x, y } => {
                let vy = *self.vx(y);
                let vx = self.vx(x);
                *vx &= vy;
                self.pc += 2;
            }
            Xorr { x, y } => {
                let vy = *self.vx(y);
                let vx = self.vx(x);
                *vx ^= vy;
                self.pc += 2;
            }
            Addr { x, y } => {
                let vy = *self.vx(y);
                let vx = *self.vx(x);
                let (res, carry) = vx.overflowing_add(vy);
                *self.vx(x) = res;
                self.v[0xF] = carry as u8;
                self.pc += 2;
            }
            Subr { x, y } => {
                let vy = *self.vx(y);
                let vx = *self.vx(x);
                let (res, overflow) = vx.overflowing_sub(vy);
                *self.vx(x) = res;
                self.v[0xF] = !overflow as u8;
                self.pc += 2;
            }
            Shrr { x, .. } => {
                let vx = *self.vx(x);
                *self.vx(x) = vx >> 1;
                self.v[0xF] = vx & 0x1;
                self.pc += 2;
            }
            Subnr { x, y } => {
                let vy = *self.vx(y);
                let vx = *self.vx(x);
                let (res, overflow) = vy.overflowing_sub(vx);
                *self.vx(x) = res;
                self.v[0xF] = !overflow as u8;
                self.pc += 2;
            }
            Shlr { x, .. } => {
                let vx = *self.vx(x);
                *self.vx(x) = vx << 1;
                self.v[0xF] = vx >> 7;
                self.pc += 2;
            }
            Sner { x, y } => {
                if *self.vx(x) != *self.vx(y) {
                    self.pc += 4;
                } else {
                    self.pc += 2;
                }
            }
            Ldi { nnn } => {
                self.i = nnn;
                self.pc += 2;
            }
            Jpo { nnn } => {
                self.pc = (nnn + (*self.vx(0) as u16)) as usize;
            }
            Rnd { x, kk } => {
                let n: u8 = random!();
                *self.vx(x) = n & kk;
                self.pc += 2;
            }
            Drw { x, y, n } => {
                let vx = *self.vx(x) as usize;
                let vy = *self.vx(y) as usize;
                let bit_off = vx & 7; // vx % 8
                let col_byte = vx >> 3; // vx / 8
                let height = n as usize;

                let rows = self.screen.len();
                let bytes_per_row = self.screen[0].len();

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
            Skp { x } => {
                let vx = *self.vx(x);
                if self.keys[(vx & 0xF) as usize] {
                    self.pc += 4
                } else {
                    self.pc += 2
                }
            }
            Sknp { x } => {
                let vx = *self.vx(x);
                if !self.keys[(vx & 0xF) as usize] {
                    self.pc += 4
                } else {
                    self.pc += 2
                }
            }
            Lddv { x } => {
                let val = *self.vx(x);
                self.dt.store(val, Ordering::Release);
                self.pc += 2;
            }
            Ldk { x } => {
                *self.vx(x) = self.dt.load(Ordering::Acquire);
                self.pc += 2;
            }
            Ldvd { x } => match self.key_state {
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
            Ldsv { x } => {
                let val = *self.vx(x);
                self.st.store(val, Ordering::Release);
                self.pc += 2;
            }
            Addi { x } => {
                let vx = *self.vx(x);
                self.i += vx as usize;
                self.pc += 2;
            }
            Ldfv { x } => {
                // set I to the 5 line high hex sprite for the lowest nibble in vX
                let vx = *self.vx(x) & 0x0F;
                self.i = (vx * 5) as usize;
                self.pc += 2;
            }
            Ldbv { x } => {
                let vx = *self.vx(x);
                self.memory[self.i] = (vx % 255) / 100;
                self.memory[self.i + 1] = (vx % 100) / 10;
                self.memory[self.i + 2] = vx % 10;
                self.pc += 2;
            }
            Ldiv { x } => {
                for vx in &mut self.v[0..=x] {
                    self.memory[self.i] = *vx;
                    self.i += 1;
                }
                self.pc += 2;
            }
            Ldvi { x } => {
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

    pub fn parseop(op: u16) -> ChipOp {
        match op & 0xF000 {
            0x0000 => match op {
                0x00E0 => ChipOp::Cls,
                0x00EE => ChipOp::Ret,
                _ => ChipOp::Unknown(op),
            },
            0x1000 => ChipOp::Jp {
                nnn: (op & 0x0FFF) as usize,
            },
            0x2000 => ChipOp::Call {
                nnn: (op & 0x0FFF) as usize,
            },
            0x3000 => ChipOp::Se {
                x: ((op & 0x0F00) >> 8) as usize,
                kk: (op & 0x00FF) as u8,
            },
            0x4000 => ChipOp::Sne {
                x: ((op & 0x0F00) >> 8) as usize,
                kk: (op & 0x00FF) as u8,
            },
            0x5000 => ChipOp::Ser {
                x: ((op & 0x0F00) >> 8) as usize,
                y: ((op & 0x00F0) >> 4) as usize,
            },
            0x6000 => ChipOp::Ld {
                x: ((op & 0x0F00) >> 8) as usize,
                kk: (op & 0x00FF) as u8,
            },
            0x7000 => ChipOp::Add {
                x: ((op & 0x0F00) >> 8) as usize,
                kk: (op & 0x00FF) as u8,
            },
            0x8000 => {
                let x = ((op & 0x0F00) >> 8) as usize;
                let y = ((op & 0x00F0) >> 4) as usize;
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
                    x: ((op & 0x0F00) >> 8) as usize,
                    y: ((op & 0x00F0) >> 4) as usize,
                },
                _ => ChipOp::Unknown(op),
            },
            0xA000 => ChipOp::Ldi {
                nnn: (op & 0x0FFF) as usize,
            },
            0xB000 => ChipOp::Jpo { nnn: op & 0x0FFF },
            0xC000 => ChipOp::Rnd {
                x: ((op & 0x0F00) >> 8) as usize,
                kk: (op & 0x00FF) as u8,
            },
            0xD000 => ChipOp::Drw {
                x: ((op & 0x0F00) >> 8) as usize,
                y: ((op & 0x00F0) >> 4) as usize,
                n: (op & 0x000F) as u8,
            },
            0xE000 => match op & 0x00FF {
                0x009E => ChipOp::Skp {
                    x: ((op & 0x0F00) >> 8) as usize,
                },
                0x00A1 => ChipOp::Sknp {
                    x: ((op & 0x0F00) >> 8) as usize,
                },
                _ => ChipOp::Unknown(op),
            },
            0xF000 => {
                let x = ((op & 0x0F00) >> 8) as usize;
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
    fn vx(&mut self, x: usize) -> &mut u8 {
        &mut self.v[x]
    }
}
