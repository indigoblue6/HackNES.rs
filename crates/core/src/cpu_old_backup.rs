//! # 6502 CPU
//!
//! NESのCPUは6502のカスタム版（2A03）。

use crate::bus::Bus;
use crate::error::Result;

/// アドレッシングモード
#[derive(Debug, Clone, Copy)]
enum Mode {
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndirectX,
    IndirectY,
    Accumulator,
    Implied,
}

/// CPU レジスタとステータス
#[derive(Debug)]
pub struct Cpu {
    pub pc: u16,
    pub sp: u8,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub status: u8,
    pub cycles: u64,
}

/// ステータスフラグ
impl Cpu {
    const CARRY: u8 = 0b0000_0001;
    const ZERO: u8 = 0b0000_0010;
    const INTERRUPT: u8 = 0b0000_0100;
    const DECIMAL: u8 = 0b0000_1000;
    const BREAK: u8 = 0b0001_0000;
    const UNUSED: u8 = 0b0010_0000;
    const OVERFLOW: u8 = 0b0100_0000;
    const NEGATIVE: u8 = 0b1000_0000;

    pub fn new() -> Self {
        Self {
            pc: 0,
            sp: 0xFD,
            a: 0,
            x: 0,
            y: 0,
            status: Self::UNUSED | Self::INTERRUPT,
            cycles: 0,
        }
    }

    pub fn reset(&mut self, bus: &mut Bus) {
        let lo = bus.cpu_read(0xFFFC) as u16;
        let hi = bus.cpu_read(0xFFFD) as u16;
        self.pc = (hi << 8) | lo;
        self.sp = 0xFD;
        self.status = Self::UNUSED | Self::INTERRUPT;
        self.cycles = 0;
        log::info!("CPU reset, PC set to: {:#06x}", self.pc);
    }

    fn get_flag(&self, flag: u8) -> bool {
        (self.status & flag) != 0
    }

    fn set_flag(&mut self, flag: u8, value: bool) {
        if value {
            self.status |= flag;
        } else {
            self.status &= !flag;
        }
    }

    fn update_zero_and_negative(&mut self, value: u8) {
        self.set_flag(Self::ZERO, value == 0);
        self.set_flag(Self::NEGATIVE, (value & 0x80) != 0);
    }

    fn push(&mut self, bus: &mut Bus, value: u8) {
        bus.cpu_write(0x0100 | (self.sp as u16), value);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn pop(&mut self, bus: &mut Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.cpu_read(0x0100 | (self.sp as u16))
    }

    fn push_word(&mut self, bus: &mut Bus, value: u16) {
        self.push(bus, (value >> 8) as u8);
        self.push(bus, value as u8);
    }

    fn pop_word(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.pop(bus) as u16;
        let hi = self.pop(bus) as u16;
        (hi << 8) | lo
    }

    // アドレッシングモードからアドレスを取得
    fn get_operand_address(&mut self, bus: &mut Bus, mode: Mode) -> u16 {
        match mode {
            Mode::Immediate => {
                let addr = self.pc;
                self.pc = self.pc.wrapping_add(1);
                addr
            }
            Mode::ZeroPage => {
                let addr = bus.cpu_read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                addr
            }
            Mode::ZeroPageX => {
                let addr = bus.cpu_read(self.pc).wrapping_add(self.x) as u16;
                self.pc = self.pc.wrapping_add(1);
                addr
            }
            Mode::ZeroPageY => {
                let addr = bus.cpu_read(self.pc).wrapping_add(self.y) as u16;
                self.pc = self.pc.wrapping_add(1);
                addr
            }
            Mode::Absolute => {
                let lo = bus.cpu_read(self.pc) as u16;
                let hi = bus.cpu_read(self.pc.wrapping_add(1)) as u16;
                self.pc = self.pc.wrapping_add(2);
                (hi << 8) | lo
            }
            Mode::AbsoluteX => {
                let lo = bus.cpu_read(self.pc) as u16;
                let hi = bus.cpu_read(self.pc.wrapping_add(1)) as u16;
                self.pc = self.pc.wrapping_add(2);
                ((hi << 8) | lo).wrapping_add(self.x as u16)
            }
            Mode::AbsoluteY => {
                let lo = bus.cpu_read(self.pc) as u16;
                let hi = bus.cpu_read(self.pc.wrapping_add(1)) as u16;
                self.pc = self.pc.wrapping_add(2);
                ((hi << 8) | lo).wrapping_add(self.y as u16)
            }
            Mode::Indirect => {
                let ptr_lo = bus.cpu_read(self.pc) as u16;
                let ptr_hi = bus.cpu_read(self.pc.wrapping_add(1)) as u16;
                self.pc = self.pc.wrapping_add(2);
                let ptr = (ptr_hi << 8) | ptr_lo;
                // 6502バグ: ページ境界を超えない
                let lo = bus.cpu_read(ptr) as u16;
                let hi = bus.cpu_read((ptr & 0xFF00) | ((ptr + 1) & 0x00FF)) as u16;
                (hi << 8) | lo
            }
            Mode::IndirectX => {
                let ptr = bus.cpu_read(self.pc).wrapping_add(self.x);
                self.pc = self.pc.wrapping_add(1);
                let lo = bus.cpu_read(ptr as u16) as u16;
                let hi = bus.cpu_read(ptr.wrapping_add(1) as u16) as u16;
                (hi << 8) | lo
            }
            Mode::IndirectY => {
                let ptr = bus.cpu_read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                let lo = bus.cpu_read(ptr as u16) as u16;
                let hi = bus.cpu_read(ptr.wrapping_add(1) as u16) as u16;
                ((hi << 8) | lo).wrapping_add(self.y as u16)
            }
            _ => panic!("Invalid addressing mode"),
        }
    }

    fn read_operand(&mut self, bus: &mut Bus, mode: Mode) -> u8 {
        let addr = self.get_operand_address(bus, mode);
        bus.cpu_read(addr)
    }

    // === 命令実装 ===

    fn lda(&mut self, bus: &mut Bus, mode: Mode) {
        let value = self.read_operand(bus, mode);
        self.a = value;
        self.update_zero_and_negative(value);
    }

    fn ldx(&mut self, bus: &mut Bus, mode: Mode) {
        let value = self.read_operand(bus, mode);
        self.x = value;
        self.update_zero_and_negative(value);
    }

    fn ldy(&mut self, bus: &mut Bus, mode: Mode) {
        let value = self.read_operand(bus, mode);
        self.y = value;
        self.update_zero_and_negative(value);
    }

    fn sta(&mut self, bus: &mut Bus, mode: Mode) {
        let addr = self.get_operand_address(bus, mode);
        bus.cpu_write(addr, self.a);
    }

    fn stx(&mut self, bus: &mut Bus, mode: Mode) {
        let addr = self.get_operand_address(bus, mode);
        bus.cpu_write(addr, self.x);
    }

    fn sty(&mut self, bus: &mut Bus, mode: Mode) {
        let addr = self.get_operand_address(bus, mode);
        bus.cpu_write(addr, self.y);
    }

    fn adc(&mut self, bus: &mut Bus, mode: Mode) {
        let value = self.read_operand(bus, mode);
        let carry = if self.get_flag(Self::CARRY) { 1 } else { 0 };
        let result = self.a as u16 + value as u16 + carry as u16;
        
        self.set_flag(Self::CARRY, result > 0xFF);
        self.set_flag(Self::OVERFLOW, 
            ((self.a ^ value) & 0x80 == 0) && ((self.a ^ result as u8) & 0x80 != 0));
        
        self.a = result as u8;
        self.update_zero_and_negative(self.a);
    }

    fn sbc(&mut self, bus: &mut Bus, mode: Mode) {
        let value = self.read_operand(bus, mode);
        let carry = if self.get_flag(Self::CARRY) { 1 } else { 0 };
        let result = self.a as i16 - value as i16 - (1 - carry) as i16;
        
        self.set_flag(Self::CARRY, result >= 0);
        self.set_flag(Self::OVERFLOW,
            ((self.a ^ value) & 0x80 != 0) && ((self.a ^ result as u8) & 0x80 != 0));
        
        self.a = result as u8;
        self.update_zero_and_negative(self.a);
    }

    fn and(&mut self, bus: &mut Bus, mode: Mode) {
        let value = self.read_operand(bus, mode);
        self.a &= value;
        self.update_zero_and_negative(self.a);
    }

    fn ora(&mut self, bus: &mut Bus, mode: Mode) {
        let value = self.read_operand(bus, mode);
        self.a |= value;
        self.update_zero_and_negative(self.a);
    }

    fn eor(&mut self, bus: &mut Bus, mode: Mode) {
        let value = self.read_operand(bus, mode);
        self.a ^= value;
        self.update_zero_and_negative(self.a);
    }

    fn cmp(&mut self, bus: &mut Bus, mode: Mode) {
        let value = self.read_operand(bus, mode);
        let result = self.a.wrapping_sub(value);
        self.set_flag(Self::CARRY, self.a >= value);
        self.update_zero_and_negative(result);
    }

    fn cpx(&mut self, bus: &mut Bus, mode: Mode) {
        let value = self.read_operand(bus, mode);
        let result = self.x.wrapping_sub(value);
        self.set_flag(Self::CARRY, self.x >= value);
        self.update_zero_and_negative(result);
    }

    fn cpy(&mut self, bus: &mut Bus, mode: Mode) {
        let value = self.read_operand(bus, mode);
        let result = self.y.wrapping_sub(value);
        self.set_flag(Self::CARRY, self.y >= value);
        self.update_zero_and_negative(result);
    }

    fn bit(&mut self, bus: &mut Bus, mode: Mode) {
        let value = self.read_operand(bus, mode);
        self.set_flag(Self::ZERO, (self.a & value) == 0);
        self.set_flag(Self::OVERFLOW, (value & 0x40) != 0);
        self.set_flag(Self::NEGATIVE, (value & 0x80) != 0);
    }

    fn asl(&mut self, bus: &mut Bus, mode: Mode) {
        if matches!(mode, Mode::Accumulator) {
            self.set_flag(Self::CARRY, (self.a & 0x80) != 0);
            self.a <<= 1;
            self.update_zero_and_negative(self.a);
        } else {
            let addr = self.get_operand_address(bus, mode);
            let mut value = bus.cpu_read(addr);
            self.set_flag(Self::CARRY, (value & 0x80) != 0);
            value <<= 1;
            bus.cpu_write(addr, value);
            self.update_zero_and_negative(value);
        }
    }

    fn lsr(&mut self, bus: &mut Bus, mode: Mode) {
        if matches!(mode, Mode::Accumulator) {
            self.set_flag(Self::CARRY, (self.a & 0x01) != 0);
            self.a >>= 1;
            self.update_zero_and_negative(self.a);
        } else {
            let addr = self.get_operand_address(bus, mode);
            let mut value = bus.cpu_read(addr);
            self.set_flag(Self::CARRY, (value & 0x01) != 0);
            value >>= 1;
            bus.cpu_write(addr, value);
            self.update_zero_and_negative(value);
        }
    }

    fn rol(&mut self, bus: &mut Bus, mode: Mode) {
        let old_carry = if self.get_flag(Self::CARRY) { 1 } else { 0 };
        if matches!(mode, Mode::Accumulator) {
            self.set_flag(Self::CARRY, (self.a & 0x80) != 0);
            self.a = (self.a << 1) | old_carry;
            self.update_zero_and_negative(self.a);
        } else {
            let addr = self.get_operand_address(bus, mode);
            let mut value = bus.cpu_read(addr);
            self.set_flag(Self::CARRY, (value & 0x80) != 0);
            value = (value << 1) | old_carry;
            bus.cpu_write(addr, value);
            self.update_zero_and_negative(value);
        }
    }

    fn ror(&mut self, bus: &mut Bus, mode: Mode) {
        let old_carry = if self.get_flag(Self::CARRY) { 0x80 } else { 0 };
        if matches!(mode, Mode::Accumulator) {
            self.set_flag(Self::CARRY, (self.a & 0x01) != 0);
            self.a = (self.a >> 1) | old_carry;
            self.update_zero_and_negative(self.a);
        } else {
            let addr = self.get_operand_address(bus, mode);
            let mut value = bus.cpu_read(addr);
            self.set_flag(Self::CARRY, (value & 0x01) != 0);
            value = (value >> 1) | old_carry;
            bus.cpu_write(addr, value);
            self.update_zero_and_negative(value);
        }
    }

    fn inc(&mut self, bus: &mut Bus, mode: Mode) {
        let addr = self.get_operand_address(bus, mode);
        let value = bus.cpu_read(addr).wrapping_add(1);
        bus.cpu_write(addr, value);
        self.update_zero_and_negative(value);
    }

    fn dec(&mut self, bus: &mut Bus, mode: Mode) {
        let addr = self.get_operand_address(bus, mode);
        let value = bus.cpu_read(addr).wrapping_sub(1);
        bus.cpu_write(addr, value);
        self.update_zero_and_negative(value);
    }

    fn branch(&mut self, condition: bool, bus: &mut Bus) {
        let offset = bus.cpu_read(self.pc) as i8;
        self.pc = self.pc.wrapping_add(1);
        if condition {
            self.pc = self.pc.wrapping_add(offset as u16);
        }
    }

    pub fn step(&mut self, bus: &mut Bus) -> Result<u8> {
        let opcode = bus.cpu_read(self.pc);
        self.pc = self.pc.wrapping_add(1);

        let cycles = match opcode {
            // LDA
            0xA9 => { self.lda(bus, Mode::Immediate); 2 }
            0xA5 => { self.lda(bus, Mode::ZeroPage); 3 }
            0xB5 => { self.lda(bus, Mode::ZeroPageX); 4 }
            0xAD => { self.lda(bus, Mode::Absolute); 4 }
            0xBD => { self.lda(bus, Mode::AbsoluteX); 4 }
            0xB9 => { self.lda(bus, Mode::AbsoluteY); 4 }
            0xA1 => { self.lda(bus, Mode::IndirectX); 6 }
            0xB1 => { self.lda(bus, Mode::IndirectY); 5 }

            // LDX
            0xA2 => { self.ldx(bus, Mode::Immediate); 2 }
            0xA6 => { self.ldx(bus, Mode::ZeroPage); 3 }
            0xB6 => { self.ldx(bus, Mode::ZeroPageY); 4 }
            0xAE => { self.ldx(bus, Mode::Absolute); 4 }
            0xBE => { self.ldx(bus, Mode::AbsoluteY); 4 }

            // LDY
            0xA0 => { self.ldy(bus, Mode::Immediate); 2 }
            0xA4 => { self.ldy(bus, Mode::ZeroPage); 3 }
            0xB4 => { self.ldy(bus, Mode::ZeroPageX); 4 }
            0xAC => { self.ldy(bus, Mode::Absolute); 4 }
            0xBC => { self.ldy(bus, Mode::AbsoluteX); 4 }

            // STA
            0x85 => { self.sta(bus, Mode::ZeroPage); 3 }
            0x95 => { self.sta(bus, Mode::ZeroPageX); 4 }
            0x8D => { self.sta(bus, Mode::Absolute); 4 }
            0x9D => { self.sta(bus, Mode::AbsoluteX); 5 }
            0x99 => { self.sta(bus, Mode::AbsoluteY); 5 }
            0x81 => { self.sta(bus, Mode::IndirectX); 6 }
            0x91 => { self.sta(bus, Mode::IndirectY); 6 }

            // STX
            0x86 => { self.stx(bus, Mode::ZeroPage); 3 }
            0x96 => { self.stx(bus, Mode::ZeroPageY); 4 }
            0x8E => { self.stx(bus, Mode::Absolute); 4 }

            // STY
            0x84 => { self.sty(bus, Mode::ZeroPage); 3 }
            0x94 => { self.sty(bus, Mode::ZeroPageX); 4 }
            0x8C => { self.sty(bus, Mode::Absolute); 4 }

            // ADC
            0x69 => { self.adc(bus, Mode::Immediate); 2 }
            0x65 => { self.adc(bus, Mode::ZeroPage); 3 }
            0x75 => { self.adc(bus, Mode::ZeroPageX); 4 }
            0x6D => { self.adc(bus, Mode::Absolute); 4 }
            0x7D => { self.adc(bus, Mode::AbsoluteX); 4 }
            0x79 => { self.adc(bus, Mode::AbsoluteY); 4 }
            0x61 => { self.adc(bus, Mode::IndirectX); 6 }
            0x71 => { self.adc(bus, Mode::IndirectY); 5 }

            // SBC
            0xE9 => { self.sbc(bus, Mode::Immediate); 2 }
            0xE5 => { self.sbc(bus, Mode::ZeroPage); 3 }
            0xF5 => { self.sbc(bus, Mode::ZeroPageX); 4 }
            0xED => { self.sbc(bus, Mode::Absolute); 4 }
            0xFD => { self.sbc(bus, Mode::AbsoluteX); 4 }
            0xF9 => { self.sbc(bus, Mode::AbsoluteY); 4 }
            0xE1 => { self.sbc(bus, Mode::IndirectX); 6 }
            0xF1 => { self.sbc(bus, Mode::IndirectY); 5 }

            // AND
            0x29 => { self.and(bus, Mode::Immediate); 2 }
            0x25 => { self.and(bus, Mode::ZeroPage); 3 }
            0x35 => { self.and(bus, Mode::ZeroPageX); 4 }
            0x2D => { self.and(bus, Mode::Absolute); 4 }
            0x3D => { self.and(bus, Mode::AbsoluteX); 4 }
            0x39 => { self.and(bus, Mode::AbsoluteY); 4 }
            0x21 => { self.and(bus, Mode::IndirectX); 6 }
            0x31 => { self.and(bus, Mode::IndirectY); 5 }

            // ORA
            0x09 => { self.ora(bus, Mode::Immediate); 2 }
            0x05 => { self.ora(bus, Mode::ZeroPage); 3 }
            0x15 => { self.ora(bus, Mode::ZeroPageX); 4 }
            0x0D => { self.ora(bus, Mode::Absolute); 4 }
            0x1D => { self.ora(bus, Mode::AbsoluteX); 4 }
            0x19 => { self.ora(bus, Mode::AbsoluteY); 4 }
            0x01 => { self.ora(bus, Mode::IndirectX); 6 }
            0x11 => { self.ora(bus, Mode::IndirectY); 5 }

            // EOR
            0x49 => { self.eor(bus, Mode::Immediate); 2 }
            0x45 => { self.eor(bus, Mode::ZeroPage); 3 }
            0x55 => { self.eor(bus, Mode::ZeroPageX); 4 }
            0x4D => { self.eor(bus, Mode::Absolute); 4 }
            0x5D => { self.eor(bus, Mode::AbsoluteX); 4 }
            0x59 => { self.eor(bus, Mode::AbsoluteY); 4 }
            0x41 => { self.eor(bus, Mode::IndirectX); 6 }
            0x51 => { self.eor(bus, Mode::IndirectY); 5 }

            // CMP
            0xC9 => { self.cmp(bus, Mode::Immediate); 2 }
            0xC5 => { self.cmp(bus, Mode::ZeroPage); 3 }
            0xD5 => { self.cmp(bus, Mode::ZeroPageX); 4 }
            0xCD => { self.cmp(bus, Mode::Absolute); 4 }
            0xDD => { self.cmp(bus, Mode::AbsoluteX); 4 }
            0xD9 => { self.cmp(bus, Mode::AbsoluteY); 4 }
            0xC1 => { self.cmp(bus, Mode::IndirectX); 6 }
            0xD1 => { self.cmp(bus, Mode::IndirectY); 5 }

            // CPX
            0xE0 => { self.cpx(bus, Mode::Immediate); 2 }
            0xE4 => { self.cpx(bus, Mode::ZeroPage); 3 }
            0xEC => { self.cpx(bus, Mode::Absolute); 4 }

            // CPY
            0xC0 => { self.cpy(bus, Mode::Immediate); 2 }
            0xC4 => { self.cpy(bus, Mode::ZeroPage); 3 }
            0xCC => { self.cpy(bus, Mode::Absolute); 4 }

            // BIT
            0x24 => { self.bit(bus, Mode::ZeroPage); 3 }
            0x2C => { self.bit(bus, Mode::Absolute); 4 }

            // ASL
            0x0A => { self.asl(bus, Mode::Accumulator); 2 }
            0x06 => { self.asl(bus, Mode::ZeroPage); 5 }
            0x16 => { self.asl(bus, Mode::ZeroPageX); 6 }
            0x0E => { self.asl(bus, Mode::Absolute); 6 }
            0x1E => { self.asl(bus, Mode::AbsoluteX); 7 }

            // LSR
            0x4A => { self.lsr(bus, Mode::Accumulator); 2 }
            0x46 => { self.lsr(bus, Mode::ZeroPage); 5 }
            0x56 => { self.lsr(bus, Mode::ZeroPageX); 6 }
            0x4E => { self.lsr(bus, Mode::Absolute); 6 }
            0x5E => { self.lsr(bus, Mode::AbsoluteX); 7 }

            // ROL
            0x2A => { self.rol(bus, Mode::Accumulator); 2 }
            0x26 => { self.rol(bus, Mode::ZeroPage); 5 }
            0x36 => { self.rol(bus, Mode::ZeroPageX); 6 }
            0x2E => { self.rol(bus, Mode::Absolute); 6 }
            0x3E => { self.rol(bus, Mode::AbsoluteX); 7 }

            // ROR
            0x6A => { self.ror(bus, Mode::Accumulator); 2 }
            0x66 => { self.ror(bus, Mode::ZeroPage); 5 }
            0x76 => { self.ror(bus, Mode::ZeroPageX); 6 }
            0x6E => { self.ror(bus, Mode::Absolute); 6 }
            0x7E => { self.ror(bus, Mode::AbsoluteX); 7 }

            // INC
            0xE6 => { self.inc(bus, Mode::ZeroPage); 5 }
            0xF6 => { self.inc(bus, Mode::ZeroPageX); 6 }
            0xEE => { self.inc(bus, Mode::Absolute); 6 }
            0xFE => { self.inc(bus, Mode::AbsoluteX); 7 }

            // DEC
            0xC6 => { self.dec(bus, Mode::ZeroPage); 5 }
            0xD6 => { self.dec(bus, Mode::ZeroPageX); 6 }
            0xCE => { self.dec(bus, Mode::Absolute); 6 }
            0xDE => { self.dec(bus, Mode::AbsoluteX); 7 }

            // INX, INY, DEX, DEY
            0xE8 => { self.x = self.x.wrapping_add(1); self.update_zero_and_negative(self.x); 2 }
            0xC8 => { self.y = self.y.wrapping_add(1); self.update_zero_and_negative(self.y); 2 }
            0xCA => { self.x = self.x.wrapping_sub(1); self.update_zero_and_negative(self.x); 2 }
            0x88 => { self.y = self.y.wrapping_sub(1); self.update_zero_and_negative(self.y); 2 }

            // Transfers
            0xAA => { self.x = self.a; self.update_zero_and_negative(self.x); 2 }
            0xA8 => { self.y = self.a; self.update_zero_and_negative(self.y); 2 }
            0x8A => { self.a = self.x; self.update_zero_and_negative(self.a); 2 }
            0x98 => { self.a = self.y; self.update_zero_and_negative(self.a); 2 }
            0x9A => { self.sp = self.x; 2 }
            0xBA => { self.x = self.sp; self.update_zero_and_negative(self.x); 2 }

            // Stack
            0x48 => { self.push(bus, self.a); 3 }
            0x08 => { self.push(bus, self.status | Self::BREAK | Self::UNUSED); 3 }
            0x68 => { self.a = self.pop(bus); self.update_zero_and_negative(self.a); 4 }
            0x28 => { 
                self.status = self.pop(bus);
                self.status &= !Self::BREAK;
                self.status |= Self::UNUSED;
                4
            }

            // Branches
            0x10 => { self.branch((self.status & Self::NEGATIVE) == 0, bus); 2 }
            0x30 => { self.branch((self.status & Self::NEGATIVE) != 0, bus); 2 }
            0x50 => { self.branch((self.status & Self::OVERFLOW) == 0, bus); 2 }
            0x70 => { self.branch((self.status & Self::OVERFLOW) != 0, bus); 2 }
            0x90 => { self.branch((self.status & Self::CARRY) == 0, bus); 2 }
            0xB0 => { self.branch((self.status & Self::CARRY) != 0, bus); 2 }
            0xD0 => { self.branch((self.status & Self::ZERO) == 0, bus); 2 }
            0xF0 => { self.branch((self.status & Self::ZERO) != 0, bus); 2 }

            // Jumps
            0x4C => { self.pc = self.get_operand_address(bus, Mode::Absolute); 3 }
            0x6C => { self.pc = self.get_operand_address(bus, Mode::Indirect); 5 }

            // JSR / RTS
            0x20 => {
                let target = self.get_operand_address(bus, Mode::Absolute);
                self.push_word(bus, self.pc.wrapping_sub(1));
                self.pc = target;
                6
            }
            0x60 => { self.pc = self.pop_word(bus).wrapping_add(1); 6 }

            // BRK / RTI
            0x00 => {
                self.pc = self.pc.wrapping_add(1);
                self.push_word(bus, self.pc);
                self.push(bus, self.status | Self::BREAK | Self::UNUSED);
                self.set_flag(Self::INTERRUPT, true);
                let lo = bus.cpu_read(0xFFFE) as u16;
                let hi = bus.cpu_read(0xFFFF) as u16;
                self.pc = (hi << 8) | lo;
                7
            }
            0x40 => {
                self.status = self.pop(bus);
                self.status &= !Self::BREAK;
                self.status |= Self::UNUSED;
                self.pc = self.pop_word(bus);
                6
            }

            // Flags
            0x18 => { self.set_flag(Self::CARRY, false); 2 }
            0x38 => { self.set_flag(Self::CARRY, true); 2 }
            0x58 => { self.set_flag(Self::INTERRUPT, false); 2 }
            0x78 => { self.set_flag(Self::INTERRUPT, true); 2 }
            0xB8 => { self.set_flag(Self::OVERFLOW, false); 2 }
            0xD8 => { self.set_flag(Self::DECIMAL, false); 2 }
            0xF8 => { self.set_flag(Self::DECIMAL, true); 2 }

            // NOP
            0xEA => 2,

            _ => {
                log::warn!("Unimplemented opcode: {:#04x} at PC: {:#06x}", opcode, self.pc.wrapping_sub(1));
                2
            }
        };

        self.cycles += cycles as u64;
        Ok(cycles)
    }

    pub fn pc(&self) -> u16 {
        self.pc
    }

    pub fn sp(&self) -> u8 {
        self.sp
    }

    pub fn a(&self) -> u8 {
        self.a
    }

    pub fn x(&self) -> u8 {
        self.x
    }

    pub fn y(&self) -> u8 {
        self.y
    }

    pub fn status(&self) -> u8 {
        self.status
    }

    pub fn cycles(&self) -> u64 {
        self.cycles
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}
