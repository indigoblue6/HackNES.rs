//! # 6502 CPU Implementation
//! Based on https://github.com/starrhorne/nes-rust

use crate::bus::Bus;

#[derive(Debug, Copy, Clone, PartialEq)]
enum Flag {
    Carry      = 0b00000001,
    Zero       = 0b00000010,
    IrqDisable = 0b00000100,
    Decimal    = 0b00001000,
    Break      = 0b00010000,
    Push       = 0b00100000,
    Overflow   = 0b01000000,
    Negative   = 0b10000000,
}

#[allow(dead_code)]
#[derive(Debug, Copy, Clone, PartialEq)]
enum Mode {
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteXForceTick,
    AbsoluteY,
    AbsoluteYForceTick,
    Indirect,
    IndirectX,
    IndirectY,
    IndirectYForceTick,
    NoMode,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Interrupt {
    Nmi,
    Reset,
    Irq,
    Break,
}

pub struct Cpu {
    pub bus: Bus,
    pc: u16,
    sp: u8,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub p: u8,
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            bus: Bus::new(),
            pc: 0,
            sp: 0,
            a: 0,
            x: 0,
            y: 0,
            p: 0,
        }
    }

    pub fn reset(&mut self) {
        self.sp = 0xFF;
        self.p = 0x34;
        self.interrupt(Interrupt::Reset);
    }

    pub fn pc(&self) -> u16 {
        self.pc
    }

    pub fn sp(&self) -> u8 {
        self.sp
    }

    pub fn status(&self) -> u8 {
        self.p
    }

    fn next_byte(&mut self) -> u8 {
        let value = self.bus.read_byte(self.pc);
        self.increment_pc();
        value
    }

    fn next_word(&mut self) -> u16 {
        let lo = self.next_byte() as u16;
        let hi = self.next_byte() as u16;
        (hi << 8) | lo
    }

    fn increment_pc(&mut self) {
        self.pc = self.pc.wrapping_add(1);
    }

    fn pop_byte(&mut self) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        let address = 0x100 + self.sp as u16;
        self.bus.read_byte(address)
    }

    fn push_byte(&mut self, value: u8) {
        let address = 0x100 + self.sp as u16;
        self.bus.write_byte(address, value);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn pop_word(&mut self) -> u16 {
        let lo = self.pop_byte() as u16;
        let hi = self.pop_byte() as u16;
        (hi << 8) | lo
    }

    fn push_word(&mut self, value: u16) {
        self.push_byte((value >> 8) as u8);
        self.push_byte(value as u8);
    }

    fn get_flag(&self, flag: Flag) -> bool {
        (self.p & flag as u8) != 0
    }

    fn set_flag(&mut self, flag: Flag, value: bool) {
        if value {
            self.p |= flag as u8;
        } else {
            self.p &= !(flag as u8);
        }
    }

    fn set_flags_zero_negative(&mut self, value: u8) {
        self.set_flag(Flag::Zero, value == 0);
        self.set_flag(Flag::Negative, value & 0b1000_0000 != 0);
    }

    fn set_flags_carry_overflow(&mut self, m: u8, n: u8, result: u16) {
        self.set_flag(Flag::Carry, result > 0xFF);
        self.set_flag(
            Flag::Overflow,
            (m ^ result as u8) & (n ^ result as u8) & 0x80 != 0,
        );
    }

    fn carry(&self) -> u8 {
        if self.get_flag(Flag::Carry) {
            1
        } else {
            0
        }
    }

    fn operand_address(&mut self, mode: Mode) -> u16 {
        match mode {
            Mode::Immediate => {
                let original_pc = self.pc;
                self.increment_pc();
                original_pc
            }
            Mode::ZeroPage => self.next_byte() as u16,
            Mode::ZeroPageX => {
                self.bus.tick();
                low_byte(offset(self.next_byte(), self.x))
            }
            Mode::ZeroPageY => {
                self.bus.tick();
                low_byte(offset(self.next_byte(), self.y))
            }
            Mode::Absolute => self.next_word(),
            Mode::AbsoluteX => {
                let base = self.next_word();
                if cross(base, self.x) {
                    self.bus.tick();
                };
                offset(base, self.x)
            }
            Mode::AbsoluteXForceTick => {
                self.bus.tick();
                offset(self.next_word(), self.x)
            }
            Mode::AbsoluteY => {
                let base = self.next_word();
                if cross(base, self.y) {
                    self.bus.tick();
                }
                offset(base, self.y)
            }
            Mode::AbsoluteYForceTick => {
                self.bus.tick();
                offset(self.next_word(), self.y)
            }
            Mode::Indirect => {
                let i = self.next_word();
                self.bus
                    .read_noncontinuous_word(i, high_byte(i) | low_byte(i + 1))
            }
            Mode::IndirectX => {
                self.bus.tick();
                let i = offset(self.next_byte(), self.x);
                self.bus
                    .read_noncontinuous_word(low_byte(i), low_byte(i + 1))
            }
            Mode::IndirectY => {
                let i = self.next_byte();
                let base = self.bus.read_noncontinuous_word(i, low_byte(i + 1));
                if cross(base, self.y) {
                    self.bus.tick();
                }
                offset(base, self.y)
            }
            Mode::IndirectYForceTick => {
                let i = self.next_byte();
                let base = self.bus.read_noncontinuous_word(i, low_byte(i + 1));
                self.bus.tick();
                offset(base, self.y)
            }
            Mode::NoMode => panic!("Mode::NoMode should never be used to read from memory"),
        }
    }

    fn read_operand(&mut self, mode: Mode) -> u8 {
        let address = self.operand_address(mode);
        self.bus.read_byte(address)
    }

    pub fn interrupt(&mut self, kind: Interrupt) {
        if kind != Interrupt::Break && kind != Interrupt::Reset {
            self.bus.tick();
        }

        match kind {
            Interrupt::Break | Interrupt::Reset => {
                self.push_word(self.pc);
            }
            Interrupt::Nmi | Interrupt::Irq => {
                self.push_word(self.pc);
            }
        }

        let irq_disable = kind == Interrupt::Irq && self.get_flag(Flag::IrqDisable);

        if kind != Interrupt::Reset && !irq_disable {
            let mut status = self.p | Flag::Push as u8;
            match kind {
                Interrupt::Break => status |= Flag::Break as u8,
                _ => status &= !(Flag::Break as u8),
            }
            self.push_byte(status);
        }

        self.set_flag(Flag::IrqDisable, true);

        if kind != Interrupt::Reset && !irq_disable {
            let vector = match kind {
                Interrupt::Nmi => 0xFFFA,
                Interrupt::Irq | Interrupt::Break => 0xFFFE,
                _ => unreachable!(),
            };
            self.pc = self.bus.read_word(vector as u16);
        } else if kind == Interrupt::Reset {
            self.bus.tick();
            self.bus.tick();
            self.bus.tick();
            self.pc = self.bus.read_word(0xFFFC_u16);
        }
    }

    pub fn step(&mut self) -> crate::Result<()> {
        let pc = self.pc;
        let opcode = self.next_byte();
        
        // nestestデバッグ用ログ（特定のPCでのみ出力）
        if pc == 0xC66E {
            log::info!("nestest: Reached success address $C66E");
        }
        
        self.execute_instruction(opcode);
        Ok(())
    }

    pub fn execute_next_instruction(&mut self) {
        let opcode = self.next_byte();
        self.execute_instruction(opcode);
    }

    fn execute_instruction(&mut self, opcode: u8) {
        match opcode {
            // Loads
            0xa1 => self.lda(Mode::IndirectX),
            0xa5 => self.lda(Mode::ZeroPage),
            0xa9 => self.lda(Mode::Immediate),
            0xad => self.lda(Mode::Absolute),
            0xb1 => self.lda(Mode::IndirectY),
            0xb5 => self.lda(Mode::ZeroPageX),
            0xb9 => self.lda(Mode::AbsoluteY),
            0xbd => self.lda(Mode::AbsoluteX),

            0xa2 => self.ldx(Mode::Immediate),
            0xa6 => self.ldx(Mode::ZeroPage),
            0xb6 => self.ldx(Mode::ZeroPageY),
            0xae => self.ldx(Mode::Absolute),
            0xbe => self.ldx(Mode::AbsoluteY),

            0xa0 => self.ldy(Mode::Immediate),
            0xa4 => self.ldy(Mode::ZeroPage),
            0xb4 => self.ldy(Mode::ZeroPageX),
            0xac => self.ldy(Mode::Absolute),
            0xbc => self.ldy(Mode::AbsoluteX),

            // Stores
            0x85 => self.sta(Mode::ZeroPage),
            0x95 => self.sta(Mode::ZeroPageX),
            0x8d => self.sta(Mode::Absolute),
            0x9d => self.sta(Mode::AbsoluteXForceTick),
            0x99 => self.sta(Mode::AbsoluteYForceTick),
            0x81 => self.sta(Mode::IndirectX),
            0x91 => self.sta(Mode::IndirectYForceTick),

            0x86 => self.stx(Mode::ZeroPage),
            0x96 => self.stx(Mode::ZeroPageY),
            0x8e => self.stx(Mode::Absolute),

            0x84 => self.sty(Mode::ZeroPage),
            0x94 => self.sty(Mode::ZeroPageX),
            0x8c => self.sty(Mode::Absolute),

            // Arithmetic
            0x69 => self.adc(Mode::Immediate),
            0x65 => self.adc(Mode::ZeroPage),
            0x75 => self.adc(Mode::ZeroPageX),
            0x6d => self.adc(Mode::Absolute),
            0x7d => self.adc(Mode::AbsoluteX),
            0x79 => self.adc(Mode::AbsoluteY),
            0x61 => self.adc(Mode::IndirectX),
            0x71 => self.adc(Mode::IndirectY),

            0xe9 => self.sbc(Mode::Immediate),
            0xe5 => self.sbc(Mode::ZeroPage),
            0xf5 => self.sbc(Mode::ZeroPageX),
            0xed => self.sbc(Mode::Absolute),
            0xfd => self.sbc(Mode::AbsoluteX),
            0xf9 => self.sbc(Mode::AbsoluteY),
            0xe1 => self.sbc(Mode::IndirectX),
            0xf1 => self.sbc(Mode::IndirectY),

            // Comparisons
            0xc9 => self.cmp(Mode::Immediate),
            0xc5 => self.cmp(Mode::ZeroPage),
            0xd5 => self.cmp(Mode::ZeroPageX),
            0xcd => self.cmp(Mode::Absolute),
            0xdd => self.cmp(Mode::AbsoluteX),
            0xd9 => self.cmp(Mode::AbsoluteY),
            0xc1 => self.cmp(Mode::IndirectX),
            0xd1 => self.cmp(Mode::IndirectY),

            0xe0 => self.cpx(Mode::Immediate),
            0xe4 => self.cpx(Mode::ZeroPage),
            0xec => self.cpx(Mode::Absolute),

            0xc0 => self.cpy(Mode::Immediate),
            0xc4 => self.cpy(Mode::ZeroPage),
            0xcc => self.cpy(Mode::Absolute),

            // Bitwise operations
            0x29 => self.and(Mode::Immediate),
            0x25 => self.and(Mode::ZeroPage),
            0x35 => self.and(Mode::ZeroPageX),
            0x2d => self.and(Mode::Absolute),
            0x3d => self.and(Mode::AbsoluteX),
            0x39 => self.and(Mode::AbsoluteY),
            0x21 => self.and(Mode::IndirectX),
            0x31 => self.and(Mode::IndirectY),

            0x09 => self.ora(Mode::Immediate),
            0x05 => self.ora(Mode::ZeroPage),
            0x15 => self.ora(Mode::ZeroPageX),
            0x0d => self.ora(Mode::Absolute),
            0x1d => self.ora(Mode::AbsoluteX),
            0x19 => self.ora(Mode::AbsoluteY),
            0x01 => self.ora(Mode::IndirectX),
            0x11 => self.ora(Mode::IndirectY),

            0x49 => self.eor(Mode::Immediate),
            0x45 => self.eor(Mode::ZeroPage),
            0x55 => self.eor(Mode::ZeroPageX),
            0x4d => self.eor(Mode::Absolute),
            0x5d => self.eor(Mode::AbsoluteX),
            0x59 => self.eor(Mode::AbsoluteY),
            0x41 => self.eor(Mode::IndirectX),
            0x51 => self.eor(Mode::IndirectY),

            0x24 => self.bit(Mode::ZeroPage),
            0x2c => self.bit(Mode::Absolute),

            // Shifts and rotates
            0x2a => self.rol_a(),
            0x26 => self.rol(Mode::ZeroPage),
            0x36 => self.rol(Mode::ZeroPageX),
            0x2e => self.rol(Mode::Absolute),
            0x3e => self.rol(Mode::AbsoluteXForceTick),

            0x6a => self.ror_a(),
            0x66 => self.ror(Mode::ZeroPage),
            0x76 => self.ror(Mode::ZeroPageX),
            0x6e => self.ror(Mode::Absolute),
            0x7e => self.ror(Mode::AbsoluteXForceTick),

            0x0a => self.asl_a(),
            0x06 => self.asl(Mode::ZeroPage),
            0x16 => self.asl(Mode::ZeroPageX),
            0x0e => self.asl(Mode::Absolute),
            0x1e => self.asl(Mode::AbsoluteXForceTick),

            0x4a => self.lsr_a(),
            0x46 => self.lsr(Mode::ZeroPage),
            0x56 => self.lsr(Mode::ZeroPageX),
            0x4e => self.lsr(Mode::Absolute),
            0x5e => self.lsr(Mode::AbsoluteXForceTick),

            // Increments and decrements
            0xe6 => self.inc(Mode::ZeroPage),
            0xf6 => self.inc(Mode::ZeroPageX),
            0xee => self.inc(Mode::Absolute),
            0xfe => self.inc(Mode::AbsoluteXForceTick),

            0xc6 => self.dec(Mode::ZeroPage),
            0xd6 => self.dec(Mode::ZeroPageX),
            0xce => self.dec(Mode::Absolute),
            0xde => self.dec(Mode::AbsoluteXForceTick),

            0xe8 => self.inx(),
            0xca => self.dex(),
            0xc8 => self.iny(),
            0x88 => self.dey(),

            // Register moves
            0xaa => self.tax(),
            0xa8 => self.tay(),
            0x8a => self.txa(),
            0x98 => self.tya(),
            0x9a => self.txs(),
            0xba => self.tsx(),

            // Flag operations
            0x18 => self.clc(),
            0x38 => self.sec(),
            0x58 => self.cli(),
            0x78 => self.sei(),
            0xb8 => self.clv(),
            0xd8 => self.cld(),
            0xf8 => self.sed(),

            // Branches
            0x10 => self.bpl(),
            0x30 => self.bmi(),
            0x50 => self.bvc(),
            0x70 => self.bvs(),
            0x90 => self.bcc(),
            0xb0 => self.bcs(),
            0xd0 => self.bne(),
            0xf0 => self.beq(),

            // Jumps
            0x4c => self.jmp(Mode::Absolute),
            0x6c => self.jmp(Mode::Indirect),

            // Procedure calls
            0x20 => self.jsr(),
            0x60 => self.rts(),
            0x00 => self.brk(),
            0x40 => self.rti(),

            // Stack operations
            0x48 => self.pha(),
            0x68 => self.pla(),
            0x08 => self.php(),
            0x28 => self.plp(),

            // No operation
            0xea => self.nop(),
            
            // Illegal/Undocumented opcodes (mostly NOPs)
            0x1a | 0x3a | 0x5a | 0x7a | 0xda | 0xfa => self.nop(), // Implied NOP
            0x80 | 0x82 | 0x89 | 0xc2 | 0xe2 => self.nop_immediate(), // Immediate NOP
            0x04 | 0x44 | 0x64 => self.nop_zero_page(), // Zero Page NOP
            0x14 | 0x34 | 0x54 | 0x74 | 0xd4 | 0xf4 => self.nop_zero_page_x(), // Zero Page,X NOP
            0x0c => self.nop_absolute(), // Absolute NOP
            0x1c | 0x3c | 0x5c | 0x7c | 0xdc | 0xfc => self.nop_absolute_x(), // Absolute,X NOP
            
            // LAX - Load A and X (illegal opcode)
            0xa7 => self.lax(Mode::ZeroPage),
            0xb7 => self.lax(Mode::ZeroPageY),
            0xaf => self.lax(Mode::Absolute),
            0xbf => self.lax(Mode::AbsoluteY),
            0xa3 => self.lax(Mode::IndirectX),
            0xb3 => self.lax(Mode::IndirectY),
            
            // SAX - Store A AND X (illegal opcode)
            0x87 => self.sax(Mode::ZeroPage),
            0x97 => self.sax(Mode::ZeroPageY),
            0x8f => self.sax(Mode::Absolute),
            0x83 => self.sax(Mode::IndirectX),
            
            // SBC - Subtract with Carry (illegal 0xEB)
            0xeb => self.sbc(Mode::Immediate),
            
            // DCP - Decrement then Compare (illegal opcode)
            0xc7 => self.dcp(Mode::ZeroPage),
            0xd7 => self.dcp(Mode::ZeroPageX),
            0xcf => self.dcp(Mode::Absolute),
            0xdf => self.dcp(Mode::AbsoluteX),
            0xdb => self.dcp(Mode::AbsoluteY),
            0xc3 => self.dcp(Mode::IndirectX),
            0xd3 => self.dcp(Mode::IndirectY),
            
            // ISB/ISC - Increment then Subtract with Carry (illegal opcode)
            0xe7 => self.isb(Mode::ZeroPage),
            0xf7 => self.isb(Mode::ZeroPageX),
            0xef => self.isb(Mode::Absolute),
            0xff => self.isb(Mode::AbsoluteX),
            0xfb => self.isb(Mode::AbsoluteY),
            0xe3 => self.isb(Mode::IndirectX),
            0xf3 => self.isb(Mode::IndirectY),
            
            // SLO - Shift Left then OR (illegal opcode)
            0x07 => self.slo(Mode::ZeroPage),
            0x17 => self.slo(Mode::ZeroPageX),
            0x0f => self.slo(Mode::Absolute),
            0x1f => self.slo(Mode::AbsoluteX),
            0x1b => self.slo(Mode::AbsoluteY),
            0x03 => self.slo(Mode::IndirectX),
            0x13 => self.slo(Mode::IndirectY),
            
            // RLA - Rotate Left then AND (illegal opcode)
            0x27 => self.rla(Mode::ZeroPage),
            0x37 => self.rla(Mode::ZeroPageX),
            0x2f => self.rla(Mode::Absolute),
            0x3f => self.rla(Mode::AbsoluteX),
            0x3b => self.rla(Mode::AbsoluteY),
            0x23 => self.rla(Mode::IndirectX),
            0x33 => self.rla(Mode::IndirectY),
            
            // SRE - Shift Right then EOR (illegal opcode)
            0x47 => self.sre(Mode::ZeroPage),
            0x57 => self.sre(Mode::ZeroPageX),
            0x4f => self.sre(Mode::Absolute),
            0x5f => self.sre(Mode::AbsoluteX),
            0x5b => self.sre(Mode::AbsoluteY),
            0x43 => self.sre(Mode::IndirectX),
            0x53 => self.sre(Mode::IndirectY),
            
            // RRA - Rotate Right then Add with Carry (illegal opcode)
            0x67 => self.rra(Mode::ZeroPage),
            0x77 => self.rra(Mode::ZeroPageX),
            0x6f => self.rra(Mode::Absolute),
            0x7f => self.rra(Mode::AbsoluteX),
            0x7b => self.rra(Mode::AbsoluteY),
            0x63 => self.rra(Mode::IndirectX),
            0x73 => self.rra(Mode::IndirectY),

            _ => {
                log::warn!("Unimplemented instruction: 0x{:02X} at PC: 0x{:04X}", opcode, self.pc - 1);
                self.nop();
            }
        }
    }

    // Instruction implementations
    fn lda(&mut self, mode: Mode) {
        let operand = self.read_operand(mode);
        self.set_flags_zero_negative(operand);
        self.a = operand;
    }

    fn ldx(&mut self, mode: Mode) {
        let operand = self.read_operand(mode);
        self.set_flags_zero_negative(operand);
        self.x = operand;
    }

    fn ldy(&mut self, mode: Mode) {
        let operand = self.read_operand(mode);
        self.set_flags_zero_negative(operand);
        self.y = operand;
    }

    fn sta(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let value = self.a;
        self.bus.write_byte(address, value);
    }

    fn stx(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let value = self.x;
        self.bus.write_byte(address, value);
    }

    fn sty(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let value = self.y;
        self.bus.write_byte(address, value);
    }

    fn adc(&mut self, mode: Mode) {
        let operand = self.read_operand(mode);
        let a = self.a;
        let result = a as u16 + operand as u16 + self.carry() as u16;
        self.set_flags_carry_overflow(a, operand, result);
        self.set_flags_zero_negative(result as u8);
        self.a = result as u8;
    }

    fn sbc(&mut self, mode: Mode) {
        let operand = !self.read_operand(mode);
        let a = self.a;
        let result = a as u16 + operand as u16 + self.carry() as u16;
        self.set_flags_carry_overflow(a, operand, result);
        self.set_flags_zero_negative(result as u8);
        self.a = result as u8;
    }

    fn cmp(&mut self, mode: Mode) {
        let operand = self.read_operand(mode);
        let a = self.a;
        self.set_flags_zero_negative(a.wrapping_sub(operand));
        self.set_flag(Flag::Carry, a >= operand);
    }

    fn cpx(&mut self, mode: Mode) {
        let operand = self.read_operand(mode);
        let x = self.x;
        self.set_flags_zero_negative(x.wrapping_sub(operand));
        self.set_flag(Flag::Carry, x >= operand);
    }

    fn cpy(&mut self, mode: Mode) {
        let operand = self.read_operand(mode);
        let y = self.y;
        self.set_flags_zero_negative(y.wrapping_sub(operand));
        self.set_flag(Flag::Carry, y >= operand);
    }

    fn and(&mut self, mode: Mode) {
        let operand = self.read_operand(mode);
        let result = self.a & operand;
        self.set_flags_zero_negative(result);
        self.a = result;
    }

    fn ora(&mut self, mode: Mode) {
        let operand = self.read_operand(mode);
        let result = self.a | operand;
        self.set_flags_zero_negative(result);
        self.a = result;
    }

    fn eor(&mut self, mode: Mode) {
        let operand = self.read_operand(mode);
        let result = self.a ^ operand;
        self.set_flags_zero_negative(result);
        self.a = result;
    }

    fn bit(&mut self, mode: Mode) {
        let operand = self.read_operand(mode);
        self.set_flag(Flag::Zero, self.a & operand == 0);
        self.set_flag(Flag::Overflow, operand & 0b0100_0000 != 0);
        self.set_flag(Flag::Negative, operand & 0b1000_0000 != 0);
    }

    fn rol(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let operand = self.bus.read_byte(address);
        let carry = if self.get_flag(Flag::Carry) { 1 } else { 0 };
        let result = (operand << 1) | carry;
        self.bus.tick();
        self.set_flag(Flag::Carry, operand & 0b1000_0000 != 0);
        self.set_flags_zero_negative(result);
        self.bus.write_byte(address, result);
    }

    fn rol_a(&mut self) {
        let carry = if self.get_flag(Flag::Carry) { 1 } else { 0 };
        let result = (self.a << 1) | carry;
        self.bus.tick();
        self.set_flag(Flag::Carry, self.a & 0b1000_0000 != 0);
        self.set_flags_zero_negative(result);
        self.a = result;
    }

    fn ror(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let operand = self.bus.read_byte(address);
        let carry = if self.get_flag(Flag::Carry) { 0b1000_0000 } else { 0 };
        let result = (operand >> 1) | carry;
        self.bus.tick();
        self.set_flag(Flag::Carry, operand & 0b0000_0001 != 0);
        self.set_flags_zero_negative(result);
        self.bus.write_byte(address, result);
    }

    fn ror_a(&mut self) {
        let carry = if self.get_flag(Flag::Carry) { 0b1000_0000 } else { 0 };
        let result = (self.a >> 1) | carry;
        self.bus.tick();
        self.set_flag(Flag::Carry, self.a & 0b0000_0001 != 0);
        self.set_flags_zero_negative(result);
        self.a = result;
    }

    fn asl(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let operand = self.bus.read_byte(address);
        let result = operand << 1;
        self.bus.tick();
        self.set_flag(Flag::Carry, operand & 0b1000_0000 != 0);
        self.set_flags_zero_negative(result);
        self.bus.write_byte(address, result);
    }

    fn asl_a(&mut self) {
        let result = self.a << 1;
        self.bus.tick();
        self.set_flag(Flag::Carry, self.a & 0b1000_0000 != 0);
        self.set_flags_zero_negative(result);
        self.a = result;
    }

    fn lsr(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let operand = self.bus.read_byte(address);
        let result = operand >> 1;
        self.bus.tick();
        self.set_flag(Flag::Carry, operand & 0b0000_0001 != 0);
        self.set_flags_zero_negative(result);
        self.bus.write_byte(address, result);
    }

    fn lsr_a(&mut self) {
        let result = self.a >> 1;
        self.bus.tick();
        self.set_flag(Flag::Carry, self.a & 0b0000_0001 != 0);
        self.set_flags_zero_negative(result);
        self.a = result;
    }

    fn inc(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let operand = self.bus.read_byte(address);
        let result = operand.wrapping_add(1);
        self.bus.tick();
        self.set_flags_zero_negative(result);
        self.bus.write_byte(address, result);
    }

    fn dec(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let operand = self.bus.read_byte(address);
        let result = operand.wrapping_sub(1);
        self.bus.tick();
        self.set_flags_zero_negative(result);
        self.bus.write_byte(address, result);
    }

    fn inx(&mut self) {
        let result = self.x.wrapping_add(1);
        self.bus.tick();
        self.set_flags_zero_negative(result);
        self.x = result;
    }

    fn dex(&mut self) {
        let result = self.x.wrapping_sub(1);
        self.bus.tick();
        self.set_flags_zero_negative(result);
        self.x = result;
    }

    fn iny(&mut self) {
        let result = self.y.wrapping_add(1);
        self.bus.tick();
        self.set_flags_zero_negative(result);
        self.y = result;
    }

    fn dey(&mut self) {
        let result = self.y.wrapping_sub(1);
        self.bus.tick();
        self.set_flags_zero_negative(result);
        self.y = result;
    }

    fn tax(&mut self) {
        let result = self.a;
        self.bus.tick();
        self.set_flags_zero_negative(result);
        self.x = result;
    }

    fn tay(&mut self) {
        let result = self.a;
        self.bus.tick();
        self.set_flags_zero_negative(result);
        self.y = result;
    }

    fn txa(&mut self) {
        let result = self.x;
        self.bus.tick();
        self.set_flags_zero_negative(result);
        self.a = result;
    }

    fn tya(&mut self) {
        let result = self.y;
        self.bus.tick();
        self.set_flags_zero_negative(result);
        self.a = result;
    }

    fn txs(&mut self) {
        self.sp = self.x;
        self.bus.tick();
    }

    fn tsx(&mut self) {
        let result = self.sp;
        self.bus.tick();
        self.set_flags_zero_negative(result);
        self.x = result;
    }

    fn clc(&mut self) {
        self.set_flag(Flag::Carry, false);
        self.bus.tick();
    }

    fn sec(&mut self) {
        self.set_flag(Flag::Carry, true);
        self.bus.tick();
    }

    fn cli(&mut self) {
        self.set_flag(Flag::IrqDisable, false);
        self.bus.tick();
    }

    fn sei(&mut self) {
        self.set_flag(Flag::IrqDisable, true);
        self.bus.tick();
    }

    fn clv(&mut self) {
        self.set_flag(Flag::Overflow, false);
        self.bus.tick();
    }

    fn cld(&mut self) {
        self.set_flag(Flag::Decimal, false);
        self.bus.tick();
    }

    fn sed(&mut self) {
        self.set_flag(Flag::Decimal, true);
        self.bus.tick();
    }

    fn branch(&mut self, condition: bool) {
        let offset = self.next_byte() as i8;
        if condition {
            self.bus.tick();
            let target = self.pc.wrapping_add(offset as u16);
            if cross(self.pc, offset as u8) {
                self.bus.tick();
            }
            self.pc = target;
        }
    }

    fn bpl(&mut self) {
        let negative = self.get_flag(Flag::Negative);
        self.branch(!negative);
    }

    fn bmi(&mut self) {
        let negative = self.get_flag(Flag::Negative);
        self.branch(negative);
    }

    fn bvc(&mut self) {
        let overflow = self.get_flag(Flag::Overflow);
        self.branch(!overflow);
    }

    fn bvs(&mut self) {
        let overflow = self.get_flag(Flag::Overflow);
        self.branch(overflow);
    }

    fn bcc(&mut self) {
        let carry = self.get_flag(Flag::Carry);
        self.branch(!carry);
    }

    fn bcs(&mut self) {
        let carry = self.get_flag(Flag::Carry);
        self.branch(carry);
    }

    fn bne(&mut self) {
        let zero = self.get_flag(Flag::Zero);
        self.branch(!zero);
    }

    fn beq(&mut self) {
        let zero = self.get_flag(Flag::Zero);
        self.branch(zero);
    }

    fn jmp(&mut self, mode: Mode) {
        self.pc = self.operand_address(mode);
    }

    fn jsr(&mut self) {
        let target_address = self.operand_address(Mode::Absolute);
        let return_address = self.pc - 1;
        self.bus.tick();
        self.push_word(return_address);
        self.pc = target_address;
    }

    fn rts(&mut self) {
        self.bus.tick();
        self.bus.tick();
        self.pc = self.pop_word() + 1;
        self.bus.tick();
    }

    fn brk(&mut self) {
        self.pc += 1;
        self.interrupt(Interrupt::Break);
    }

    fn rti(&mut self) {
        self.p = self.pop_byte();
        self.pc = self.pop_word();
    }

    fn pha(&mut self) {
        self.bus.tick();
        let a = self.a;
        self.push_byte(a);
    }

    fn pla(&mut self) {
        self.bus.tick();
        self.bus.tick();
        let result = self.pop_byte();
        self.set_flags_zero_negative(result);
        self.a = result;
    }

    fn php(&mut self) {
        self.bus.tick();
        let p = self.p | Flag::Break as u8 | Flag::Push as u8;
        self.push_byte(p);
    }

    fn plp(&mut self) {
        self.bus.tick();
        self.bus.tick();
        self.p = self.pop_byte() & !(Flag::Break as u8) | Flag::Push as u8;
    }

    fn nop(&mut self) {
        self.bus.tick();
    }

    // Illegal/Undocumented NOP variants
    fn nop_immediate(&mut self) {
        self.next_byte(); // Read and discard immediate value
        self.bus.tick();
    }

    fn nop_zero_page(&mut self) {
        self.next_byte(); // Read and discard zero page address
        self.bus.tick();
        self.bus.tick();
    }

    fn nop_zero_page_x(&mut self) {
        let addr = self.next_byte();
        self.bus.tick();
        self.bus.tick();
        self.bus.read_byte(addr.wrapping_add(self.x) as u16); // Dummy read
    }

    fn nop_absolute(&mut self) {
        self.next_word(); // Read and discard absolute address
        self.bus.tick();
        self.bus.tick();
        self.bus.tick();
    }

    fn nop_absolute_x(&mut self) {
        let addr = self.next_word();
        let final_addr = addr.wrapping_add(self.x as u16);
        if cross(addr, self.x) {
            self.bus.tick();
        }
        self.bus.read_byte(final_addr); // Dummy read
        self.bus.tick();
        self.bus.tick();
    }
    
    // LAX - Load A and X (illegal opcode)
    fn lax(&mut self, mode: Mode) {
        let value = self.read_operand(mode);
        self.a = value;
        self.x = value;
        self.set_flags_zero_negative(value);
    }
    
    // SAX - Store A AND X (illegal opcode)
    fn sax(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let value = self.a & self.x;
        self.bus.write_byte(address, value);
    }
    
    // DCP - Decrement then Compare (illegal opcode)
    fn dcp(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let operand = self.bus.read_byte(address);
        let result = operand.wrapping_sub(1);
        self.bus.tick();
        self.bus.write_byte(address, result);
        
        // Compare with A
        self.set_flags_zero_negative(self.a.wrapping_sub(result));
        self.set_flag(Flag::Carry, self.a >= result);
    }
    
    // ISB/ISC - Increment then Subtract with Carry (illegal opcode)
    fn isb(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let operand = self.bus.read_byte(address);
        let result = operand.wrapping_add(1);
        self.bus.tick();
        self.bus.write_byte(address, result);
        
        // SBC with result
        let operand = !result;
        let a = self.a;
        let result = a as u16 + operand as u16 + self.carry() as u16;
        self.set_flags_carry_overflow(a, operand, result);
        self.set_flags_zero_negative(result as u8);
        self.a = result as u8;
    }
    
    // SLO - Shift Left then OR (illegal opcode)
    fn slo(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let operand = self.bus.read_byte(address);
        let result = operand << 1;
        self.bus.tick();
        self.set_flag(Flag::Carry, operand & 0b1000_0000 != 0);
        self.bus.write_byte(address, result);
        
        // OR with A
        let result = self.a | result;
        self.set_flags_zero_negative(result);
        self.a = result;
    }
    
    // RLA - Rotate Left then AND (illegal opcode)
    fn rla(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let operand = self.bus.read_byte(address);
        let carry = if self.get_flag(Flag::Carry) { 1 } else { 0 };
        let result = (operand << 1) | carry;
        self.bus.tick();
        self.set_flag(Flag::Carry, operand & 0b1000_0000 != 0);
        self.bus.write_byte(address, result);
        
        // AND with A
        let result = self.a & result;
        self.set_flags_zero_negative(result);
        self.a = result;
    }
    
    // SRE - Shift Right then EOR (illegal opcode)
    fn sre(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let operand = self.bus.read_byte(address);
        let result = operand >> 1;
        self.bus.tick();
        self.set_flag(Flag::Carry, operand & 0b0000_0001 != 0);
        self.bus.write_byte(address, result);
        
        // EOR with A
        let result = self.a ^ result;
        self.set_flags_zero_negative(result);
        self.a = result;
    }
    
    // RRA - Rotate Right then Add with Carry (illegal opcode)
    fn rra(&mut self, mode: Mode) {
        let address = self.operand_address(mode);
        let operand = self.bus.read_byte(address);
        let carry = if self.get_flag(Flag::Carry) { 0b1000_0000 } else { 0 };
        let result = (operand >> 1) | carry;
        self.bus.tick();
        self.set_flag(Flag::Carry, operand & 0b0000_0001 != 0);
        self.bus.write_byte(address, result);
        
        // ADC with result
        let a = self.a;
        let result_adc = a as u16 + result as u16 + self.carry() as u16;
        self.set_flags_carry_overflow(a, result, result_adc);
        self.set_flags_zero_negative(result_adc as u8);
        self.a = result_adc as u8;
    }
}

fn cross(base: u16, offset: u8) -> bool {
    (base & 0xFF) + offset as u16 > 0xFF
}

fn offset<T: Into<u16>>(base: T, offset: u8) -> u16 {
    base.into().wrapping_add(offset as u16)
}

fn low_byte<T: Into<u16>>(value: T) -> u16 {
    value.into() & 0xFF
}

fn high_byte(value: u16) -> u16 {
    value & 0xFF00
}
