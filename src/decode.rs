use crate::op::ChipOp;

pub fn decode(op: u16) -> ChipOp {
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
