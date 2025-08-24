use crate::op::ChipOp;

pub fn decode(op: u16) -> ChipOp {
    match op & 0xF000 {
        0x0000 => match op {
            0x00E0 => ChipOp::Cls,
            0x00EE => ChipOp::Ret,
            0x00FD => ChipOp::Exit,
            0x00FE => ChipOp::LowRes,
            0x00FF => ChipOp::HighRes,
            _ => ChipOp::Unknown(op),
        },
        0x1000 => ChipOp::JpNnn {
            nnn: (op & 0x0FFF) as usize,
        },
        0x2000 => ChipOp::CallNnn {
            nnn: (op & 0x0FFF) as usize,
        },
        0x3000 => ChipOp::SeVxNn {
            x: ((op & 0x0F00) >> 8) as usize,
            nn: (op & 0x00FF) as u8,
        },
        0x4000 => ChipOp::SneVxNn {
            x: ((op & 0x0F00) >> 8) as usize,
            nn: (op & 0x00FF) as u8,
        },
        0x5000 => match op & 0x000F {
            0x0000 => ChipOp::SeVxVy {
                x: ((op & 0x0F00) >> 8) as usize,
                y: ((op & 0x00F0) >> 4) as usize,
            },
            _ => ChipOp::Unknown(op),
        },
        0x6000 => ChipOp::LdVxNn {
            x: ((op & 0x0F00) >> 8) as usize,
            nn: (op & 0x00FF) as u8,
        },
        0x7000 => ChipOp::AddVxNn {
            x: ((op & 0x0F00) >> 8) as usize,
            nn: (op & 0x00FF) as u8,
        },
        0x8000 => {
            let x = ((op & 0x0F00) >> 8) as usize;
            let y = ((op & 0x00F0) >> 4) as usize;
            match op & 0x000F {
                0x0000 => ChipOp::LdVxVy { x, y },
                0x0001 => ChipOp::OrVxVy { x, y },
                0x0002 => ChipOp::AndVxVy { x, y },
                0x0003 => ChipOp::XorVxVy { x, y },
                0x0004 => ChipOp::AddVxVy { x, y },
                0x0005 => ChipOp::SubVxVy { x, y },
                0x0006 => ChipOp::ShrVxVy { x, y },
                0x0007 => ChipOp::SubnVxVy { x, y },
                0x000E => ChipOp::ShlVxVy { x, y },
                _ => ChipOp::Unknown(op),
            }
        }
        0x9000 => match op & 0x000F {
            0x0000 => ChipOp::SneVxVy {
                x: ((op & 0x0F00) >> 8) as usize,
                y: ((op & 0x00F0) >> 4) as usize,
            },
            _ => ChipOp::Unknown(op),
        },
        0xA000 => ChipOp::LdINnn {
            nnn: (op & 0x0FFF) as usize,
        },
        0xB000 => ChipOp::JpV0Nnn { nnn: op & 0x0FFF },
        0xC000 => ChipOp::RndVxNn {
            x: ((op & 0x0F00) >> 8) as usize,
            nn: (op & 0x00FF) as u8,
        },
        0xD000 => ChipOp::DrwVxVyN {
            x: ((op & 0x0F00) >> 8) as usize,
            y: ((op & 0x00F0) >> 4) as usize,
            n: (op & 0x000F) as u8,
        },
        0xE000 => match op & 0x00FF {
            0x009E => ChipOp::SkpVx {
                x: ((op & 0x0F00) >> 8) as usize,
            },
            0x00A1 => ChipOp::SknpVx {
                x: ((op & 0x0F00) >> 8) as usize,
            },
            _ => ChipOp::Unknown(op),
        },
        0xF000 => {
            let x = ((op & 0x0F00) >> 8) as usize;
            match op & 0x00FF {
                0x0015 => ChipOp::LdDtVx { x },
                0x0007 => ChipOp::LdVxDt { x },
                0x000A => ChipOp::LdVxK { x },
                0x0018 => ChipOp::LdStVx { x },
                0x001E => ChipOp::AddIVx { x },
                0x0029 => ChipOp::LdFVx { x },
                0x0033 => ChipOp::LdBVx { x },
                0x0055 => ChipOp::LdIVx { x },
                0x0065 => ChipOp::LdVxI { x },
                _ => ChipOp::Unknown(op),
            }
        }
        _ => ChipOp::Unknown(op),
    }
}
