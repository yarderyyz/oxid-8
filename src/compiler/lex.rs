// # CHIP-8 Opcodes Reference
//
// This document provides concrete examples of all CHIP-8 opcodes and their formatted output.
//
// ## System Instructions
//
// - `SCD 4` - Scroll display down 4 pixels
// - `SCU 2` - Scroll display up 2 pixels
// - `CLS` - Clear the display screen
// - `RET` - Return from subroutine
// - `SCR` - Scroll display right 4 pixels
// - `SCL` - Scroll display left 4 pixels
// - `EXIT` - Exit interpreter
// - `HIGH` - Enable high resolution mode (64x64)
// - `LOW` - Enable low resolution mode (64x32)
//
// ## Jump and Call Instructions
//
// - `JP 0x0200` - Jump to address 0x200
// - `CALL 0x02A0` - Call subroutine at address 0x2A0
// - `JP V0, 0x0200` - Jump to address 0x200 + V0
//
// ## Skip Instructions
//
// - `SE V5, 0x20` - Skip next instruction if V5 equals 0x20
// - `SNE V3, 0x40` - Skip next instruction if V3 doesn't equal 0x40
// - `SE V2, V7` - Skip next instruction if V2 equals V7
// - `SNE V1, V4` - Skip next instruction if V1 doesn't equal V4
//
// ## Load Instructions
//
// ### Basic Load Operations
// - `LD V2, 0x50` - Load 0x50 into register V2
// - `LD V3, V8` - Copy V8 into V3
// - `LD I, 0x0300` - Load address 0x300 into I register
//
// ### Timer Operations
// - `LD V6, DT` - Load delay timer value into V6
// - `LD DT, V2` - Set delay timer to V2 value
// - `LD ST, V5` - Set sound timer to V5 value
//
// ### Input Operations
// - `LD V4, K` - Wait for keypress and store in V4
//
// ### Sprite and Memory Operations
// - `LD F, V3` - Set I to sprite location for digit in V3
// - `LD B, V7` - Store BCD of V7 at I, I+1, I+2
// - `LD [I], V4` - Store V0-V4 in memory starting at I
// - `LD V6, [I]` - Load V0-V6 from memory starting at I
// - `LD [I],V2-V5` - Store V2 through V5 in memory starting at I
// - `LD V1-V3,[I]` - Load V1 through V3 from memory starting at I
//
// ## Arithmetic Instructions
//
// - `ADD V4, 0x15` - Add 0x15 to V4
// - `ADD V2, V6` - Add V6 to V2 (VF = carry)
// - `SUB V5, V3` - Subtract V3 from V5 (VF = NOT borrow)
// - `SUBN V1, V9` - Set V1 = V9 - V1 (VF = NOT borrow)
// - `ADD I, V8` - Add V8 to I register
//
// ## Logical Instructions
//
// - `OR V3, V5` - Bitwise OR of V3 and V5, store in V3
// - `AND V2, V7` - Bitwise AND of V2 and V7, store in V2
// - `XOR V4, V1` - Bitwise XOR of V4 and V1, store in V4
//
// ## Shift Instructions
//
// - `SHR V6, V2` - Right shift V2, store in V6 (VF = bit shifted out)
// - `SHL V3, V8` - Left shift V8, store in V3 (VF = bit shifted out)
//
// ## Graphics and Input
//
// - `DRW V2, V4, 5` - Draw 5-byte sprite at (V2, V4)
// - `RND V7, 0x0F` - Set V7 to random byte AND 0x0F
// - `SKP V5` - Skip if key in V5 is pressed
// - `SKNP V2` - Skip if key in V2 is not pressed
//
// ## Notes
//
// - All examples show the mnemonic format output by the formatter
// - Register numbers are shown in hexadecimal (0-F)
// - Memory addresses use 3-digit hex format (0x000-0xFFF)
// - Immediate values use 2-digit hex format for bytes (0x00-0xFF)
// - VF register is used as a flag register for carry/borrow operations

const OPERATORS: [&str; 2] = [",", "-"];

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    // Instructions
    Instruction(InstructionType),

    // Registers
    VRegister(u8), // V0-VF
    IRegister,     // I
    DtRegister,    // DT (Delay Timer)
    StRegister,    // ST (Sound Timer)

    // Special registers/values
    KeyRegister,  // K (key input)
    FontRegister, // F (font/sprite)
    BcdRegister,  // B (BCD)

    // Literals
    HexLiteral(u16),    // 0x200, 0x50, etc.
    DecimalLiteral(u8), // 5, 15, etc. (for sprite heights, scroll amounts)

    // Punctuation
    Comma,        // ,
    LeftBracket,  // [
    RightBracket, // ]
    Minus,        // - (for register ranges like V2-V5)

    // Whitespace and structure
    Whitespace,
    Newline,
    Comment, // ; comment text

    // Special
    Eof,
    Invalid(String), // For error handling
}

#[derive(Debug, Clone, PartialEq)]
pub enum InstructionType {
    // System instructions
    Scd,  // SCD
    Scu,  // SCU
    Cls,  // CLS
    Ret,  // RET
    Scr,  // SCR
    Scl,  // SCL
    Exit, // EXIT
    High, // HIGH
    Low,  // LOW

    // Jump and call
    Jp,   // JP
    Call, // CALL

    // Skip instructions
    Se,  // SE
    Sne, // SNE

    // Load instructions
    Ld, // LD

    // Arithmetic
    Add,  // ADD
    Sub,  // SUB
    Subn, // SUBN

    // Logical
    Or,  // OR
    And, // AND
    Xor, // XOR

    // Shift
    Shr, // SHR
    Shl, // SHL

    // Graphics and input
    Drw,  // DRW
    Rnd,  // RND
    Skp,  // SKP
    Sknp, // SKNP
}

#[derive(Debug, Clone)]
pub struct Token<'a> {
    pub token_type: TokenType,
    pub text: &'a str, // Original text
    pub line: usize,   // Line number
    pub column: usize, // Column position
}

impl<'a> Token<'a> {
    pub fn new(token_type: TokenType, text: &'a str, line: usize, column: usize) -> Self {
        Token {
            token_type,
            text,
            line,
            column,
        }
    }
}

impl TokenType {
    /* parse helpers here */
}

struct Tokens<'a> {
    parser: &'a Parser<'a>,
    line: usize,
    column: usize,
    index: usize,
}

impl<'a> Tokens<'a> {
    fn new(parser: &'a Parser) -> Tokens<'a> {
        Tokens {
            parser,
            line: 0,
            column: 0,
            index: 0,
        }
    }
}

impl<'a> Iterator for Tokens<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let raw_text = self.parser.raw_text;
        let tok_start = self.index;

        if let Some(ch) = raw_text.get(self.index..self.index + 1) {
            let in_whitespace = ch.contains(char::is_whitespace);
            let is_operator = OPERATORS.contains(&ch);

            if is_operator {
                self.index += 1;
                let tok = &raw_text[tok_start..self.index];
                print!("^{tok}$");
                if tok.to_lowercase() == "," {
                    return Some(Token::new(TokenType::Comma, tok, self.line, self.column));
                }
            }

            while self.index < raw_text.len() {
                let ch = &raw_text[self.index..self.index + 1];

                if ch.contains(char::is_whitespace) ^ in_whitespace {
                    break;
                }

                if OPERATORS.contains(&ch) {
                    break;
                }

                self.index += 1;
            }
            let tok = &raw_text[tok_start..self.index];
            print!("^{tok}$");

            if in_whitespace {
                return Some(Token::new(
                    TokenType::Whitespace,
                    tok,
                    self.line,
                    self.column,
                ));
            }

            if tok.to_lowercase() == "sne" {
                return Some(Token::new(
                    TokenType::Instruction(InstructionType::Sne),
                    tok,
                    self.line,
                    self.column,
                ));
            }

            if tok.len() == 2 || tok.len() == 3 && tok[..1].to_lowercase() == "v" {
                let reg_id = tok[1..].parse::<u8>().unwrap();
                return Some(Token::new(
                    TokenType::VRegister(reg_id),
                    tok,
                    self.line,
                    self.column,
                ));
            }
        }

        None
    }
}

struct Parser<'a> {
    raw_text: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(raw_text: &'a str) -> Parser<'a> {
        Parser { raw_text }
    }

    pub fn parse(&self) -> Tokens<'_> {
        Tokens::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_collect() {
        let test_op = "SNE V1, V4";
        let parser = Parser::new(test_op);
        let toks: Vec<Token> = parser.parse().collect();

        assert!(toks.len() == 6);
        assert!(toks[0].token_type == TokenType::Instruction(InstructionType::Sne));
        assert!(toks[1].token_type == TokenType::Whitespace);
        assert!(toks[2].token_type == TokenType::VRegister(1));
        assert!(toks[3].token_type == TokenType::Comma);
        assert!(toks[4].token_type == TokenType::Whitespace);
        assert!(toks[5].token_type == TokenType::VRegister(4));
    }

    #[test]
    fn test_parse_odd_space() {
        let test_op = "SNE    V1,    V4   ";
        let parser = Parser::new(test_op);
        let toks: Vec<Token> = parser.parse().collect();

        assert!(toks.len() == 7);
        assert!(toks[0].token_type == TokenType::Instruction(InstructionType::Sne));
        assert!(toks[1].token_type == TokenType::Whitespace);
        assert!(toks[2].token_type == TokenType::VRegister(1));
        assert!(toks[3].token_type == TokenType::Comma);
        assert!(toks[4].token_type == TokenType::Whitespace);
        assert!(toks[5].token_type == TokenType::VRegister(4));
        assert!(toks[6].token_type == TokenType::Whitespace);
    }
}
