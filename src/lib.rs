pub mod chip8 {
    pub mod audio;
    pub mod consts;
    pub mod cpu;
    pub mod decode;
    pub mod gfx;
    pub mod mem;
    pub mod op;
    pub mod timers;
}

pub mod utils {
    pub mod triple_buffer;
}

pub mod compiler {
    pub mod lex;
}
