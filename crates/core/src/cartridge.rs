//! # Cartridge
//! Based on https://github.com/starrhorne/nes-rust

pub struct Cartridge {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    prg_ram: Vec<u8>,
    chr_ram: Vec<u8>,
    mapper: u8,
    mirroring: Mirroring,
    // Mapper 2 (UxROM) state
    prg_bank: u8,
    // Mapper 3 (CNROM) state
    chr_bank: u8,
    // Mapper 1 (MMC1) state
    mmc1_shift_register: u8,
    mmc1_shift_count: u8,
    mmc1_control: u8,
    mmc1_chr_bank_0: u8,
    mmc1_chr_bank_1: u8,
    mmc1_prg_bank: u8,
    // MMC3 (Mapper 4) state
    registers: [u8; 8],
    register_select: u8,
    prg_bank_mode: bool,
    chr_bank_mode: bool,
    // Mapper 4 IRQ state
    irq_counter: u8,
    irq_reload: u8,
    irq_pending: bool,
    irq_enabled: bool,
    irq_reload_flag: bool,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    SingleScreenLower,
    SingleScreenUpper,
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
            // Mapper 2
            prg_bank: 0,
            // Mapper 3
            chr_bank: 0,
            // Mapper 1 (MMC1)
            mmc1_shift_register: 0,
            mmc1_shift_count: 0,
            mmc1_control: 0x0C, // PRG ROM mode 3, CHR ROM mode 0
            mmc1_chr_bank_0: 0,
            mmc1_chr_bank_1: 0,
            mmc1_prg_bank: 0,
            // Mapper 4 (MMC3)
            registers: [0, 0, 0, 0, 0, 0, 0, 0],
            register_select: 0,
            prg_bank_mode: false,
            chr_bank_mode: false,
            // Mapper 4 IRQ
            irq_counter: 0,
            irq_reload: 0,
            irq_pending: false,
            irq_enabled: false,
            irq_reload_flag: false,
        }
    }

    pub fn read_prg_byte(&self, addr: u16) -> u8 {
        match self.mapper {
            0 => self.mapper0_read_prg(addr),
            1 => self.mapper1_read_prg(addr),
            2 => self.mapper2_read_prg(addr),
            3 => self.mapper3_read_prg(addr),
            4 => self.mapper4_read_prg(addr),
            7 => self.mapper7_read_prg(addr),
            66 => self.mapper66_read_prg(addr),
            _ => {
                log::warn!("Unsupported mapper {}", self.mapper);
                0
            }
        }
    }

    pub fn write_prg_byte(&mut self, addr: u16, value: u8) {
        match self.mapper {
            0 => self.mapper0_write_prg(addr, value),
            1 => self.mapper1_write_prg(addr, value),
            2 => self.mapper2_write_prg(addr, value),
            3 => self.mapper3_write_prg(addr, value),
            4 => self.mapper4_write_prg(addr, value),
            7 => self.mapper7_write_prg(addr, value),
            66 => self.mapper66_write_prg(addr, value),
            _ => {}
        }
    }

    pub fn read_chr_byte(&self, addr: u16) -> u8 {
        match self.mapper {
            0 => self.mapper0_read_chr(addr),
            1 => self.mapper1_read_chr(addr),
            2 => self.mapper2_read_chr(addr),
            3 => self.mapper3_read_chr(addr),
            4 => self.mapper4_read_chr(addr),
            7 => self.mapper7_read_chr(addr),
            66 => self.mapper66_read_chr(addr),
            _ => 0,
        }
    }

    pub fn write_chr_byte(&mut self, addr: u16, value: u8) {
        match self.mapper {
            0 => self.mapper0_write_chr(addr, value),
            1 => self.mapper1_write_chr(addr, value),
            2 => self.mapper2_write_chr(addr, value),
            3 => self.mapper3_write_chr(addr, value),
            4 => self.mapper4_write_chr(addr, value),
            7 => self.mapper7_write_chr(addr, value),
            66 => self.mapper66_write_chr(addr, value),
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
            0xC000..=0xDFFF => {
                if addr & 1 == 0 {
                    // IRQ latch ($C000-$DFFE, even)
                    self.irq_reload = value;
                } else {
                    // IRQ reload ($C001-$DFFF, odd)
                    self.irq_reload_flag = true;
                    self.irq_counter = 0;
                }
            }
            0xE000..=0xFFFF => {
                if addr & 1 == 0 {
                    // IRQ disable ($E000-$FFFE, even)
                    self.irq_enabled = false;
                    self.irq_pending = false;
                } else {
                    // IRQ enable ($E001-$FFFF, odd)
                    self.irq_enabled = true;
                }
            }
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

    // Mapper 4 IRQ
    pub fn clock_irq(&mut self) {
        if self.irq_counter == 0 || self.irq_reload_flag {
            self.irq_counter = self.irq_reload;
            self.irq_reload_flag = false;
        } else {
            self.irq_counter -= 1;
        }

        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_pending = true;
        }
    }

    pub fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    pub fn acknowledge_irq(&mut self) {
        self.irq_pending = false;
    }

    // Mapper 2 (UxROM)
    fn mapper2_read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let index = (addr - 0x6000) as usize;
                self.prg_ram.get(index).copied().unwrap_or(0)
            }
            0x8000..=0xBFFF => {
                // Switchable 16KB bank
                let bank = self.prg_bank as usize;
                let offset = (addr - 0x8000) as usize;
                let index = (bank * 16384 + offset) % self.prg_rom.len().max(1);
                self.prg_rom.get(index).copied().unwrap_or(0)
            }
            0xC000..=0xFFFF => {
                // Fixed to last 16KB bank
                let last_bank = (self.prg_rom.len() / 16384).saturating_sub(1);
                let offset = (addr - 0xC000) as usize;
                let index = (last_bank * 16384 + offset) % self.prg_rom.len().max(1);
                self.prg_rom.get(index).copied().unwrap_or(0)
            }
            _ => 0,
        }
    }

    fn mapper2_write_prg(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let index = (addr - 0x6000) as usize;
                if index < self.prg_ram.len() {
                    self.prg_ram[index] = value;
                }
            }
            0x8000..=0xFFFF => {
                // Bank select
                self.prg_bank = value & 0x0F;
            }
            _ => {}
        }
    }

    fn mapper2_read_chr(&self, addr: u16) -> u8 {
        // CHR RAM only
        let index = (addr & 0x1FFF) as usize;
        self.chr_ram.get(index).copied().unwrap_or(0)
    }

    fn mapper2_write_chr(&mut self, addr: u16, value: u8) {
        // CHR RAM only
        let index = (addr & 0x1FFF) as usize;
        if index < self.chr_ram.len() {
            self.chr_ram[index] = value;
        }
    }

    // Mapper 3 (CNROM)
    fn mapper3_read_prg(&self, addr: u16) -> u8 {
        // Same as Mapper 0
        self.mapper0_read_prg(addr)
    }

    fn mapper3_write_prg(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let index = (addr - 0x6000) as usize;
                if index < self.prg_ram.len() {
                    self.prg_ram[index] = value;
                }
            }
            0x8000..=0xFFFF => {
                // CHR bank select
                self.chr_bank = value & 0x03;
            }
            _ => {}
        }
    }

    fn mapper3_read_chr(&self, addr: u16) -> u8 {
        if self.chr_rom.is_empty() {
            return self.chr_ram.get((addr & 0x1FFF) as usize).copied().unwrap_or(0);
        }
        let bank = self.chr_bank as usize;
        let offset = (addr & 0x1FFF) as usize;
        let index = (bank * 8192 + offset) % self.chr_rom.len().max(1);
        self.chr_rom.get(index).copied().unwrap_or(0)
    }

    fn mapper3_write_chr(&mut self, addr: u16, value: u8) {
        // CHR ROM, ignore writes (unless CHR RAM)
        if self.chr_rom.is_empty() {
            let index = (addr & 0x1FFF) as usize;
            if index < self.chr_ram.len() {
                self.chr_ram[index] = value;
            }
        }
    }

    // Mapper 1 (MMC1)
    fn mapper1_read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let index = (addr - 0x6000) as usize;
                self.prg_ram.get(index).copied().unwrap_or(0)
            }
            0x8000..=0xBFFF => {
                let prg_mode = (self.mmc1_control >> 2) & 0x03;
                let bank = match prg_mode {
                    0 | 1 => {
                        // 32KB mode: use lower bit of prg_bank
                        (self.mmc1_prg_bank & 0xFE) as usize
                    }
                    2 => {
                        // Fix first bank at $8000
                        0
                    }
                    3 => {
                        // Switch bank at $8000
                        self.mmc1_prg_bank as usize
                    }
                    _ => 0,
                };
                let offset = (addr - 0x8000) as usize;
                let index = (bank * 16384 + offset) % self.prg_rom.len().max(1);
                self.prg_rom.get(index).copied().unwrap_or(0)
            }
            0xC000..=0xFFFF => {
                let prg_mode = (self.mmc1_control >> 2) & 0x03;
                let bank = match prg_mode {
                    0 | 1 => {
                        // 32KB mode: use upper bank
                        (self.mmc1_prg_bank | 0x01) as usize
                    }
                    2 => {
                        // Switch bank at $C000
                        self.mmc1_prg_bank as usize
                    }
                    3 => {
                        // Fix last bank at $C000
                        (self.prg_rom.len() / 16384).saturating_sub(1)
                    }
                    _ => 0,
                };
                let offset = (addr - 0xC000) as usize;
                let index = (bank * 16384 + offset) % self.prg_rom.len().max(1);
                self.prg_rom.get(index).copied().unwrap_or(0)
            }
            _ => 0,
        }
    }

    fn mapper1_write_prg(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let index = (addr - 0x6000) as usize;
                if index < self.prg_ram.len() {
                    self.prg_ram[index] = value;
                }
            }
            0x8000..=0xFFFF => {
                if value & 0x80 != 0 {
                    // Reset shift register
                    self.mmc1_shift_register = 0;
                    self.mmc1_shift_count = 0;
                    self.mmc1_control |= 0x0C;
                } else {
                    // Shift in bit 0
                    self.mmc1_shift_register |= (value & 0x01) << self.mmc1_shift_count;
                    self.mmc1_shift_count += 1;

                    if self.mmc1_shift_count == 5 {
                        let register = (addr >> 13) & 0x03;
                        match register {
                            0 => {
                                // Control register
                                self.mmc1_control = self.mmc1_shift_register;
                                // Update mirroring
                                self.mirroring = match self.mmc1_control & 0x03 {
                                    0 | 1 => Mirroring::Horizontal, // Single screen (simplified)
                                    2 => Mirroring::Vertical,
                                    3 => Mirroring::Horizontal,
                                    _ => Mirroring::Horizontal,
                                };
                            }
                            1 => {
                                // CHR bank 0
                                self.mmc1_chr_bank_0 = self.mmc1_shift_register;
                            }
                            2 => {
                                // CHR bank 1
                                self.mmc1_chr_bank_1 = self.mmc1_shift_register;
                            }
                            3 => {
                                // PRG bank
                                self.mmc1_prg_bank = self.mmc1_shift_register & 0x0F;
                            }
                            _ => {}
                        }
                        self.mmc1_shift_register = 0;
                        self.mmc1_shift_count = 0;
                    }
                }
            }
            _ => {}
        }
    }

    fn mapper1_read_chr(&self, addr: u16) -> u8 {
        let chr_mode = (self.mmc1_control >> 4) & 0x01;
        let (bank, offset) = if chr_mode == 0 {
            // 8KB mode
            let bank = (self.mmc1_chr_bank_0 & 0x1E) as usize;
            let offset = (addr & 0x1FFF) as usize;
            (bank, offset)
        } else {
            // 4KB mode
            if addr < 0x1000 {
                let bank = self.mmc1_chr_bank_0 as usize;
                let offset = (addr & 0x0FFF) as usize;
                (bank, offset)
            } else {
                let bank = self.mmc1_chr_bank_1 as usize;
                let offset = (addr & 0x0FFF) as usize;
                (bank, offset)
            }
        };

        if self.chr_rom.is_empty() {
            // CHR RAM
            let index = if chr_mode == 0 {
                (bank * 8192 + offset) % self.chr_ram.len().max(1)
            } else {
                (bank * 4096 + offset) % self.chr_ram.len().max(1)
            };
            self.chr_ram.get(index).copied().unwrap_or(0)
        } else {
            // CHR ROM
            let index = if chr_mode == 0 {
                (bank * 8192 + offset) % self.chr_rom.len().max(1)
            } else {
                (bank * 4096 + offset) % self.chr_rom.len().max(1)
            };
            self.chr_rom.get(index).copied().unwrap_or(0)
        }
    }

    fn mapper1_write_chr(&mut self, addr: u16, value: u8) {
        if self.chr_rom.is_empty() {
            let chr_mode = (self.mmc1_control >> 4) & 0x01;
            let index = if chr_mode == 0 {
                let bank = (self.mmc1_chr_bank_0 & 0x1E) as usize;
                let offset = (addr & 0x1FFF) as usize;
                (bank * 8192 + offset) % self.chr_ram.len().max(1)
            } else if addr < 0x1000 {
                let bank = self.mmc1_chr_bank_0 as usize;
                let offset = (addr & 0x0FFF) as usize;
                (bank * 4096 + offset) % self.chr_ram.len().max(1)
            } else {
                let bank = self.mmc1_chr_bank_1 as usize;
                let offset = (addr & 0x0FFF) as usize;
                (bank * 4096 + offset) % self.chr_ram.len().max(1)
            };
            if index < self.chr_ram.len() {
                self.chr_ram[index] = value;
            }
        }
    }

    // Mapper 7 (AxROM)
    // 32KB switchable PRG, 8KB CHR RAM, single-screen mirroring
    fn mapper7_read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xFFFF => {
                // 32KB switchable bank
                let bank = (self.prg_bank & 0x07) as usize;
                let offset = (addr - 0x8000) as usize;
                let index = (bank * 32768 + offset) % self.prg_rom.len().max(1);
                self.prg_rom.get(index).copied().unwrap_or(0)
            }
            _ => 0,
        }
    }

    fn mapper7_write_prg(&mut self, addr: u16, value: u8) {
        if (0x8000..=0xFFFF).contains(&addr) {
            // Bits 0-2: PRG bank select
            self.prg_bank = value & 0x07;
            // Bit 4: Nametable select (single-screen mirroring)
            self.mirroring = if (value & 0x10) != 0 {
                Mirroring::SingleScreenUpper
            } else {
                Mirroring::SingleScreenLower
            };
        }
    }

    fn mapper7_read_chr(&self, addr: u16) -> u8 {
        // CHR RAM only
        let index = (addr & 0x1FFF) as usize;
        self.chr_ram.get(index).copied().unwrap_or(0)
    }

    fn mapper7_write_chr(&mut self, addr: u16, value: u8) {
        // CHR RAM only
        let index = (addr & 0x1FFF) as usize;
        if index < self.chr_ram.len() {
            self.chr_ram[index] = value;
        }
    }

    // Mapper 66 (GxROM)
    // 32KB switchable PRG, 8KB switchable CHR
    fn mapper66_read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xFFFF => {
                // 32KB switchable bank (bits 4-5 of bank register)
                let bank = ((self.prg_bank >> 4) & 0x03) as usize;
                let offset = (addr - 0x8000) as usize;
                let index = (bank * 32768 + offset) % self.prg_rom.len().max(1);
                self.prg_rom.get(index).copied().unwrap_or(0)
            }
            _ => 0,
        }
    }

    fn mapper66_write_prg(&mut self, addr: u16, value: u8) {
        if (0x8000..=0xFFFF).contains(&addr) {
            // Bits 4-5: PRG bank, Bits 0-1: CHR bank
            self.prg_bank = value;
            self.chr_bank = value & 0x03;
        }
    }

    fn mapper66_read_chr(&self, addr: u16) -> u8 {
        if self.chr_rom.is_empty() {
            return self.chr_ram.get((addr & 0x1FFF) as usize).copied().unwrap_or(0);
        }
        // 8KB switchable bank
        let bank = self.chr_bank as usize;
        let offset = (addr & 0x1FFF) as usize;
        let index = (bank * 8192 + offset) % self.chr_rom.len().max(1);
        self.chr_rom.get(index).copied().unwrap_or(0)
    }

    fn mapper66_write_chr(&mut self, addr: u16, value: u8) {
        // CHR ROM, ignore writes (unless CHR RAM)
        if self.chr_rom.is_empty() {
            let index = (addr & 0x1FFF) as usize;
            if index < self.chr_ram.len() {
                self.chr_ram[index] = value;
            }
        }
    }
}

