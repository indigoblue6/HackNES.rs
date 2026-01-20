//! # Cartridge
//! Based on https://github.com/starrhorne/nes-rust

pub struct Cartridge {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    prg_ram: Vec<u8>,
    chr_ram: Vec<u8>,
    mapper: u8,
    mirroring: Mirroring,
    // MMC3 (Mapper 4) state
    registers: [u8; 8],
    register_select: u8,
    prg_bank_mode: bool,
    chr_bank_mode: bool,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
}

impl Cartridge {
    pub fn new(data: &[u8]) -> Self {
        if data.len() < 16 {
            panic!("ROM too small");
        }

        if &data[0..4] != b"NES\x1A" {
            panic!("Invalid iNES header");
        }

        let prg_rom_size = data[4] as usize * 16384;
        let chr_rom_size = data[5] as usize * 8192;
        let flags6 = data[6];
        let flags7 = data[7];

        let mapper = (flags7 & 0xF0) | (flags6 >> 4);
        let mirroring = if (flags6 & 0x01) != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };

        let has_trainer = (flags6 & 0x04) != 0;
        let prg_start = 16 + if has_trainer { 512 } else { 0 };
        let chr_start = prg_start + prg_rom_size;

        let prg_rom = data[prg_start..prg_start + prg_rom_size].to_vec();
        let chr_rom = if chr_rom_size > 0 {
            data[chr_start..chr_start + chr_rom_size].to_vec()
        } else {
            vec![]
        };
        let chr_ram = if chr_rom_size == 0 {
            vec![0; 8192]
        } else {
            vec![]
        };

        Cartridge {
            prg_rom,
            chr_rom,
            prg_ram: vec![0; 8192], // 8KB PRG RAM
            chr_ram,
            mapper,
            mirroring,
            registers: [0, 0, 0, 0, 0, 0, 0, 0],
            register_select: 0,
            prg_bank_mode: false,
            chr_bank_mode: false,
        }
    }

    pub fn read_prg_byte(&self, addr: u16) -> u8 {
        match self.mapper {
            0 => self.mapper0_read_prg(addr),
            4 => self.mapper4_read_prg(addr),
            _ => {
                log::warn!("Unsupported mapper {}", self.mapper);
                0
            }
        }
    }

    pub fn write_prg_byte(&mut self, addr: u16, value: u8) {
        match self.mapper {
            0 => self.mapper0_write_prg(addr, value),
            4 => self.mapper4_write_prg(addr, value),
            _ => {}
        }
    }

    pub fn read_chr_byte(&self, addr: u16) -> u8 {
        match self.mapper {
            0 => self.mapper0_read_chr(addr),
            4 => self.mapper4_read_chr(addr),
            _ => 0,
        }
    }

    pub fn write_chr_byte(&mut self, addr: u16, value: u8) {
        match self.mapper {
            0 => self.mapper0_write_chr(addr, value),
            4 => self.mapper4_write_chr(addr, value),
            _ => {}
        }
    }

    pub fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    // Mapper 0 (NROM)
    fn mapper0_read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let index = (addr - 0x6000) as usize;
                self.prg_ram.get(index).copied().unwrap_or(0)
            }
            0x8000..=0xFFFF => {
                let index = if self.prg_rom.len() == 16384 {
                    (addr & 0x3FFF) as usize
                } else {
                    (addr & 0x7FFF) as usize
                };
                self.prg_rom.get(index).copied().unwrap_or(0)
            }
            _ => 0,
        }
    }

    fn mapper0_write_prg(&mut self, addr: u16, value: u8) {
        if (0x6000..=0x7FFF).contains(&addr) {
            let index = (addr - 0x6000) as usize;
            if index < self.prg_ram.len() {
                self.prg_ram[index] = value;
            }
        }
    }

    fn mapper0_read_chr(&self, addr: u16) -> u8 {
        let index = (addr & 0x1FFF) as usize;
        if !self.chr_rom.is_empty() {
            self.chr_rom.get(index).copied().unwrap_or(0)
        } else {
            self.chr_ram.get(index).copied().unwrap_or(0)
        }
    }

    fn mapper0_write_chr(&mut self, addr: u16, value: u8) {
        let index = (addr & 0x1FFF) as usize;
        if self.chr_rom.is_empty() && index < self.chr_ram.len() {
            self.chr_ram[index] = value;
        }
    }

    // Mapper 4 (MMC3)
    fn mapper4_read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let index = (addr - 0x6000) as usize;
                self.prg_ram.get(index).copied().unwrap_or(0)
            }
            0x8000..=0x9FFF => {
                let bank = if self.prg_bank_mode {
                    (self.prg_rom.len() / 8192).saturating_sub(2)
                } else {
                    self.registers[6] as usize
                };
                let offset = (addr - 0x8000) as usize;
                let index = (bank * 8192 + offset) % self.prg_rom.len().max(1);
                self.prg_rom.get(index).copied().unwrap_or(0)
            }
            0xA000..=0xBFFF => {
                let bank = self.registers[7] as usize;
                let offset = (addr - 0xA000) as usize;
                let index = (bank * 8192 + offset) % self.prg_rom.len().max(1);
                self.prg_rom.get(index).copied().unwrap_or(0)
            }
            0xC000..=0xDFFF => {
                let bank = if self.prg_bank_mode {
                    self.registers[6] as usize
                } else {
                    (self.prg_rom.len() / 8192).saturating_sub(2)
                };
                let offset = (addr - 0xC000) as usize;
                let index = (bank * 8192 + offset) % self.prg_rom.len().max(1);
                self.prg_rom.get(index).copied().unwrap_or(0)
            }
            0xE000..=0xFFFF => {
                let bank = (self.prg_rom.len() / 8192).saturating_sub(1);
                let offset = (addr - 0xE000) as usize;
                let index = (bank * 8192 + offset) % self.prg_rom.len().max(1);
                self.prg_rom.get(index).copied().unwrap_or(0)
            }
            _ => 0,
        }
    }

    fn mapper4_write_prg(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let index = (addr - 0x6000) as usize;
                if index < self.prg_ram.len() {
                    self.prg_ram[index] = value;
                }
            }
            0x8000..=0x9FFF => {
                if addr & 1 == 0 {
                    // Bank select ($8000-$9FFE, even)
                    self.register_select = value & 0x07;
                    self.prg_bank_mode = (value & 0x40) != 0;
                    self.chr_bank_mode = (value & 0x80) != 0;
                } else {
                    // Bank data ($8001-$9FFF, odd)
                    let index = self.register_select as usize;
                    if index < 8 {
                        self.registers[index] = value;
                    }
                }
            }
            0xA000..=0xBFFF => {
                if addr & 1 == 0 {
                    // Mirroring ($A000-$BFFE, even)
                    self.mirroring = if (value & 0x01) != 0 {
                        Mirroring::Horizontal
                    } else {
                        Mirroring::Vertical
                    };
                }
                // PRG RAM protect ($A001-$BFFF, odd) - ignored for now
            }
            // $C000-$DFFF: IRQ registers (not implemented)
            // $E000-$FFFF: IRQ registers (not implemented)
            _ => {}
        }
    }

    fn mapper4_read_chr(&self, addr: u16) -> u8 {
        if self.chr_rom.is_empty() {
            return self.chr_ram.get((addr & 0x1FFF) as usize).copied().unwrap_or(0);
        }

        let bank = match addr {
            0x0000..=0x03FF => {
                if !self.chr_bank_mode {
                    self.registers[0] & 0xFE
                } else {
                    self.registers[2]
                }
            }
            0x0400..=0x07FF => {
                if !self.chr_bank_mode {
                    self.registers[0] | 0x01
                } else {
                    self.registers[3]
                }
            }
            0x0800..=0x0BFF => {
                if !self.chr_bank_mode {
                    self.registers[1] & 0xFE
                } else {
                    self.registers[4]
                }
            }
            0x0C00..=0x0FFF => {
                if !self.chr_bank_mode {
                    self.registers[1] | 0x01
                } else {
                    self.registers[5]
                }
            }
            0x1000..=0x13FF => {
                if !self.chr_bank_mode {
                    self.registers[2]
                } else {
                    self.registers[0] & 0xFE
                }
            }
            0x1400..=0x17FF => {
                if !self.chr_bank_mode {
                    self.registers[3]
                } else {
                    self.registers[0] | 0x01
                }
            }
            0x1800..=0x1BFF => {
                if !self.chr_bank_mode {
                    self.registers[4]
                } else {
                    self.registers[1] & 0xFE
                }
            }
            0x1C00..=0x1FFF => {
                if !self.chr_bank_mode {
                    self.registers[5]
                } else {
                    self.registers[1] | 0x01
                }
            }
            _ => 0,
        } as usize;

        let offset = (addr & 0x03FF) as usize;
        let index = (bank * 1024 + offset) % self.chr_rom.len().max(1);
        self.chr_rom.get(index).copied().unwrap_or(0)
    }

    fn mapper4_write_chr(&mut self, addr: u16, value: u8) {
        if self.chr_rom.is_empty() {
            let index = (addr & 0x1FFF) as usize;
            if index < self.chr_ram.len() {
                self.chr_ram[index] = value;
            }
        }
    }
}

