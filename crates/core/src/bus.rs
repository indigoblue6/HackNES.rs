//! # Memory Bus
//! Based on https://github.com/starrhorne/nes-rust

use crate::apu::Apu;
use crate::ppu::Ppu;
use crate::cartridge::Cartridge;
use crate::controller::Controller;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Bus {
    pub ram: [u8; 2048],
    pub ppu: Ppu,
    pub apu: Apu,
    pub cartridge: Option<Rc<RefCell<Cartridge>>>,
    pub controller: Controller,
    pub cycles: u64,
    cpu_stall_cycles: usize,
}

impl Bus {
    pub fn new() -> Self {
        Bus {
            ram: [0; 2048],
            ppu: Ppu::new(),
            apu: Apu::new(),
            cartridge: None,
            controller: Controller::new(),
            cycles: 0,
            cpu_stall_cycles: 0,
        }
    }

    pub fn reset_cpu_stall_cycles(&mut self) -> usize {
        let c = self.cpu_stall_cycles;
        self.cpu_stall_cycles = 0;
        c
    }

    fn unclocked_read_byte(&mut self, address: u16) -> u8 {
        match address {
            0x0000..=0x1FFF => self.ram[address as usize % 0x0800],
            0x2000..=0x3FFF => self.ppu.read_register(address),
            0x4000..=0x4013 => 0, // APU write-only registers
            0x4014 => 0,         // OAM DMA
            0x4015 => self.apu.read_register(address),
            0x4016 => self.controller.read(),
            0x4017 => 0, // Controller 2 (not implemented)
            0x4018..=0x401F => 0, // APU/IO test registers
            _ => {
                // 0x4020..=0xFFFF
                if let Some(ref c) = self.cartridge {
                    c.borrow().read_prg_byte(address)
                } else {
                    (address >> 8) as u8
                }
            }
        }
    }

    fn unclocked_write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => self.ram[address as usize % 0x0800] = value,
            0x2000..=0x3FFF => self.ppu.write_register(address, value),
            0x4000..=0x4013 => self.apu.write_register(address, value),
            0x4014 => self.oam_dma(value as u16),
            0x4015 => self.apu.write_register(address, value),
            0x4016 => self.controller.write(value),
            0x4017 => self.apu.write_register(address, value),
            0x4018..=0x401F => {} // APU/IO test registers (ignored)
            _ => {
                // 0x4020..=0xFFFF
                if let Some(ref c) = self.cartridge {
                    c.borrow_mut().write_prg_byte(address, value);
                }
            }
        }
    }

    fn oam_dma(&mut self, bank: u16) {
        self.cpu_stall_cycles += 513 + (self.cycles as usize % 2);
        for i in 0..256 {
            let v = self.unclocked_read_byte(bank * 0x100 + i);
            self.ppu.write_oam_data(v);
        }
    }

    pub fn read_byte<T: Into<u16>>(&mut self, address: T) -> u8 {
        self.tick();
        self.unclocked_read_byte(address.into())
    }

    pub fn write_byte<T: Into<u16>>(&mut self, address: T, value: u8) {
        self.tick();
        self.unclocked_write_byte(address.into(), value)
    }

    pub fn read_noncontinuous_word<T: Into<u16>, U: Into<u16>>(&mut self, a: T, b: U) -> u16 {
        (self.read_byte(a) as u16) | (self.read_byte(b) as u16) << 8
    }

    pub fn read_word<T: Into<u16>>(&mut self, address: T) -> u16 {
        let address = address.into();
        self.read_noncontinuous_word(address, address + 1)
    }

    pub fn tick(&mut self) {
        self.cycles += 1;

        // Tick PPU 3 times for every CPU cycle
        for _ in 0..3 {
            self.ppu.tick();
        }

        // Tick APU once per CPU cycle
        self.apu.tick();
    }

    pub fn load_rom_from_memory(&mut self, data: &[u8]) {
        let c = Rc::new(RefCell::new(Cartridge::new(data)));
        self.ppu.set_cartridge(c.clone());
        self.cartridge = Some(c);
    }

    pub fn reset(&mut self) {
        // Reset state
    }
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}
