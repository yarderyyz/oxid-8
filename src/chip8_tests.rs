#[cfg(test)]
mod tests {
    use crate::types::Chip8;
    use crate::types::ChipOp;

    #[test]
    fn test_run_op_ret() {
        let pc = 0x200;
        let mut chip: Chip8 = Default::default();
        chip.stack[0] = pc;
        chip.sp = 1;
        chip.pc = 0xABC;

        chip.run_op(ChipOp::Ret);
        assert!(chip.sp == 0);
        assert!(chip.pc == pc);
    }

    #[test]
    fn test_run_op_jp() {
        let pc = 0x200;
        let mut chip: Chip8 = Default::default();
        let op = ChipOp::Jp { nnn: pc };
        chip.run_op(op);
        assert!(chip.pc == pc);
    }

    #[test]
    fn test_run_op_call() {
        let addr = 0xABC;
        let mut chip: Chip8 = Default::default();

        chip.run_op(ChipOp::Call { nnn: addr });
        assert!(chip.sp == 1);
        assert!(chip.pc == addr);
    }

    #[test]
    fn test_run_op_se_skip() {
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[0] = 20;

        chip.run_op(ChipOp::Se { x: 0, kk: 20 });
        assert!(chip.pc == 0x204);
    }

    #[test]
    fn test_run_op_se_no_skip() {
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[1] = 10;

        chip.run_op(ChipOp::Se { x: 1, kk: 20 });
        assert!(chip.pc == 0x202);
    }

    #[test]
    fn test_run_op_sne_no_skip() {
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[0] = 20;

        chip.run_op(ChipOp::Sne { x: 0, kk: 20 });
        assert!(chip.pc == 0x202);
    }

    #[test]
    fn test_run_op_sne_skip() {
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[1] = 10;

        chip.run_op(ChipOp::Sne { x: 1, kk: 20 });
        assert!(chip.pc == 0x204);
    }

    #[test]
    fn test_run_op_ser_skip() {
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[0] = 20;
        chip.v[1] = 20;

        chip.run_op(ChipOp::Ser { x: 0, y: 1 });
        assert!(chip.pc == 0x204);
    }

    #[test]
    fn test_run_op_ser_no_skip() {
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[0] = 20;
        chip.v[1] = 17;

        chip.run_op(ChipOp::Ser { x: 0, y: 1 });
        assert!(chip.pc == 0x202);
    }

    #[test]
    fn test_run_op_ld() {
        let reg: u8 = 3;
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };

        chip.run_op(ChipOp::Ld { x: reg, kk: 0xAB });
        assert!(chip.v[reg as usize] == 0xAB);
    }

    #[test]
    fn test_run_op_add() {
        let reg: u8 = 3;
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };

        chip.run_op(ChipOp::Add { x: reg, kk: 0xA0 });
        assert!(chip.v[reg as usize] == 0xA0);

        chip.run_op(ChipOp::Add { x: reg, kk: 0x0B });
        assert!(chip.v[reg as usize] == 0xAB);
    }

    #[test]
    fn test_run_op_ldr() {
        let x: u8 = 3;
        let y: u8 = 5;
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[y as usize] = 0xAB;

        chip.run_op(ChipOp::Ldr { x, y });
        assert!(chip.v[x as usize] == 0xAB);
    }

    #[test]
    fn test_run_drw_row() {
        let img_loc = 0x400;
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[0] = 0;
        chip.v[1] = 0;
        chip.i = img_loc;
        chip.memory[img_loc as usize] = 0xAB;
        chip.run_op(ChipOp::Drw { x: 0, y: 1, n: 1 });
        assert!(chip.screen[0][0] == 0xAB);
    }

    #[test]
    fn test_run_drw_row_x_offset() {
        let img_loc = 0x400;
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[0] = 1;
        chip.v[1] = 0;
        chip.i = img_loc;
        chip.memory[img_loc as usize] = 0b11110000;
        chip.run_op(ChipOp::Drw { x: 0, y: 1, n: 1 });
        println!("{:#010b}", chip.screen[0][0]);
        assert!(chip.screen[0][0] == 0b01111000);
    }

    #[test]
    fn test_run_drw_row_x_offset_byte_boundary() {
        let img_loc = 0x400;
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[0] = 6;
        chip.v[1] = 0;
        chip.i = img_loc;
        chip.memory[img_loc as usize] = 0b11110000;
        chip.run_op(ChipOp::Drw { x: 0, y: 1, n: 1 });
        assert!(chip.screen[0][0] == 0b00000011);
        assert!(chip.screen[0][1] == 0b11000000);
    }

    #[test]
    fn test_run_drw_row_x_offset_big() {
        let img_loc = 0x400;
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[0] = 13;
        chip.v[1] = 0;
        chip.i = img_loc;
        chip.memory[img_loc as usize] = 0b11110000;
        chip.run_op(ChipOp::Drw { x: 0, y: 1, n: 1 });
        assert!(chip.screen[0][1] == 0b00000111);
        assert!(chip.screen[0][2] == 0b10000000);
    }

    #[test]
    fn test_run_drw_zero() {
        let img_loc: usize = 0x400;
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[0] = 0;
        chip.v[1] = 0;
        chip.i = img_loc as u16;
        chip.memory[img_loc] = 0xF0;
        chip.memory[img_loc + 1] = 0x90;
        chip.memory[img_loc + 2] = 0x90;
        chip.memory[img_loc + 3] = 0x90;
        chip.memory[img_loc + 4] = 0xF0;
        chip.run_op(ChipOp::Drw { x: 0, y: 1, n: 5 });
        assert!(chip.screen[0][0] == 0xF0);
        assert!(chip.screen[1][0] == 0x90);
        assert!(chip.screen[2][0] == 0x90);
        assert!(chip.screen[3][0] == 0x90);
        assert!(chip.screen[4][0] == 0xF0);
    }

    #[test]
    fn test_run_drw_zero_y_offset() {
        let img_loc: usize = 0x400;
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[0] = 0;
        chip.v[1] = 1;
        chip.i = img_loc as u16;
        chip.memory[img_loc] = 0xF0;
        chip.memory[img_loc + 1] = 0x90;
        chip.memory[img_loc + 2] = 0x90;
        chip.memory[img_loc + 3] = 0x90;
        chip.memory[img_loc + 4] = 0xF0;
        chip.run_op(ChipOp::Drw { x: 0, y: 1, n: 5 });
        assert!(chip.screen[1][0] == 0xF0);
        assert!(chip.screen[2][0] == 0x90);
        assert!(chip.screen[3][0] == 0x90);
        assert!(chip.screen[4][0] == 0x90);
        assert!(chip.screen[5][0] == 0xF0);
    }

    #[test]
    fn test_run_drw_zero_xy_offset() {
        let img_loc: usize = 0x400;
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[0] = 4;
        chip.v[1] = 1;
        chip.i = img_loc as u16;
        chip.memory[img_loc] = 0xF0;
        chip.memory[img_loc + 1] = 0x90;
        chip.memory[img_loc + 2] = 0x90;
        chip.memory[img_loc + 3] = 0x90;
        chip.memory[img_loc + 4] = 0xF0;
        chip.run_op(ChipOp::Drw { x: 0, y: 1, n: 5 });
        assert!(chip.screen[1][0] == 0x0F);
        assert!(chip.screen[2][0] == 0x09);
        assert!(chip.screen[3][0] == 0x09);
        assert!(chip.screen[4][0] == 0x09);
        assert!(chip.screen[5][0] == 0x0F);
    }

    #[test]
    fn test_run_drw_test_collision() {
        let img_loc: usize = 0x400;
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };
        chip.v[0] = 4;
        chip.v[1] = 1;
        chip.i = img_loc as u16;
        chip.memory[img_loc] = 0xF0;
        chip.memory[img_loc + 1] = 0x90;
        chip.memory[img_loc + 2] = 0x90;
        chip.memory[img_loc + 3] = 0x90;
        chip.memory[img_loc + 4] = 0xF0;

        // Test first drw has no collision
        chip.run_op(ChipOp::Drw { x: 0, y: 1, n: 5 });
        assert!(chip.v[0xF] == 0);

        // Change offset and check that the collision flag is set
        chip.run_op(ChipOp::Drw { x: 0, y: 1, n: 4 });
        assert!(chip.v[0x1] == 1);
    }

    #[test]
    fn test_run_op_clr() {
        let mut chip = Chip8 {
            pc: 0x200,
            ..Chip8::default()
        };

        chip.screen[0][0] = 0xFF;
        chip.screen[10][5] = 0x0F;
        chip.v[0xF] = 1;

        chip.run_op(ChipOp::Cls);

        assert!(chip.screen.iter().all(|row| row.iter().all(|&b| b == 0)));
    }
}
