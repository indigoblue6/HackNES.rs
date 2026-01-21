//! # NES Core
//! Based on https://github.com/starrhorne/nes-rust

pub mod apu;
pub mod cpu;
pub mod ppu;
pub mod bus;
pub mod cartridge;
pub mod controller;
pub mod error;
pub mod memory_editor;

pub use error::{NesError, Result};

/// NESエミュレータのメインインスタンス
pub struct Nes {
    cpu: cpu::Cpu,
}

impl Nes {
    /// 新しいNESインスタンスを作成
    pub fn new() -> Self {
        Self {
            cpu: cpu::Cpu::new(),
        }
    }

    /// ROMをロード
    pub fn load_rom(&mut self, rom_data: &[u8]) -> Result<()> {
        self.cpu.bus.load_rom_from_memory(rom_data);
        self.cpu.reset();
        Ok(())
    }

    /// システムをリセット
    pub fn reset(&mut self) {
        self.cpu.reset();
    }

    /// 1フレーム実行（約29780.5サイクル）
    pub fn step_frame(&mut self) -> Result<&[u8]> {
        const CYCLES_PER_FRAME: u64 = 29781;
        let target = self.cpu.bus.cycles + CYCLES_PER_FRAME;

        while self.cpu.bus.cycles < target {
            // Handle OAM DMA stall cycles
            let stall_cycles = self.cpu.bus.reset_cpu_stall_cycles();
            for _ in 0..stall_cycles {
                self.cpu.bus.tick();
            }

            self.cpu.step()?;

            // nestestの成功アドレスをチェック
            if self.cpu.pc() == 0xC66E {
                log::info!("nestest PASSED at PC=$C66E");
            }

            // Check for NMI
            if self.cpu.bus.ppu.nmi {
                self.cpu.interrupt(cpu::Interrupt::Nmi);
                self.cpu.bus.ppu.nmi = false;
            }

            // Check for IRQ from mapper
            if let Some(ref c) = self.cpu.bus.cartridge {
                if c.borrow().irq_pending() {
                    c.borrow_mut().acknowledge_irq();
                    self.cpu.interrupt(cpu::Interrupt::Irq);
                }
            }

            // Check for IRQ from APU
            if self.cpu.bus.apu.irq_pending() {
                self.cpu.interrupt(cpu::Interrupt::Irq);
            }
        }

        Ok(self.cpu.bus.ppu.frame_buffer())
    }

    /// オーディオサンプルを取得
    pub fn get_audio_samples(&mut self) -> Vec<f32> {
        self.cpu.bus.apu.get_samples()
    }

    /// 1CPUサイクル実行（デバッグ用）
    pub fn step(&mut self) -> Result<u32> {
        // Handle OAM DMA stall cycles
        let stall_cycles = self.cpu.bus.reset_cpu_stall_cycles();
        for _ in 0..stall_cycles {
            self.cpu.bus.tick();
        }

        let start_cycles = self.cpu.bus.cycles;
        self.cpu.step()?;
        let elapsed = (self.cpu.bus.cycles - start_cycles) as u32;

        if self.cpu.bus.ppu.nmi {
            self.cpu.interrupt(cpu::Interrupt::Nmi);
            self.cpu.bus.ppu.nmi = false;
        }

        // Check for IRQ from mapper
        if let Some(ref c) = self.cpu.bus.cartridge {
            if c.borrow().irq_pending() {
                c.borrow_mut().acknowledge_irq();
                self.cpu.interrupt(cpu::Interrupt::Irq);
            }
        }

        // Check for IRQ from APU
        if self.cpu.bus.apu.irq_pending() {
            self.cpu.interrupt(cpu::Interrupt::Irq);
        }

        Ok(elapsed)
    }

    /// CPU状態の取得（デバッグ用）
    pub fn cpu_state(&self) -> &cpu::Cpu {
        &self.cpu
    }

    /// CPU状態のmutable取得（コントローラー入力用）
    pub fn cpu_state_mut(&mut self) -> &mut cpu::Cpu {
        &mut self.cpu
    }

    /// PPU状態の取得（デバッグ用）
    pub fn ppu_state(&self) -> &ppu::Ppu {
        &self.cpu.bus.ppu
    }

    // ========== メモリエディタ API ==========

    /// CPU RAMを読み取り (0x0000-0x07FF, 2KB)
    pub fn read_ram(&self) -> &[u8; 2048] {
        &self.cpu.bus.ram
    }

    /// CPU RAMに書き込み
    pub fn write_ram(&mut self, address: u16, value: u8) {
        let addr = (address & 0x07FF) as usize;
        self.cpu.bus.ram[addr] = value;
    }

    /// CPU RAMを一括書き込み
    pub fn write_ram_range(&mut self, start: u16, data: &[u8]) {
        for (i, &value) in data.iter().enumerate() {
            let addr = ((start as usize + i) & 0x07FF) as usize;
            self.cpu.bus.ram[addr] = value;
        }
    }

    /// 任意のCPUメモリアドレスを読み取り（バスを通じてマッピングを考慮）
    pub fn peek_memory(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x1FFF => self.cpu.bus.ram[(address & 0x07FF) as usize],
            0x2000..=0x3FFF => 0, // PPUレジスタは読み取り時に副作用があるため0を返す
            0x4000..=0x401F => 0, // APU/IOレジスタ
            _ => {
                // 0x4020..=0xFFFF
                if let Some(ref c) = self.cpu.bus.cartridge {
                    c.borrow().read_prg_byte(address)
                } else {
                    0
                }
            }
        }
    }

    /// 任意のCPUメモリアドレスに書き込み
    pub fn poke_memory(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                self.cpu.bus.ram[(address & 0x07FF) as usize] = value;
            }
            0x6000..=0x7FFF => {
                // PRG RAM
                if let Some(ref c) = self.cpu.bus.cartridge {
                    c.borrow_mut().write_prg_byte(address, value);
                }
            }
            _ => {}
        }
    }

    /// メモリ範囲を読み取り
    pub fn read_memory_range(&self, start: u16, length: usize) -> Vec<u8> {
        (0..length)
            .map(|i| self.peek_memory(start.wrapping_add(i as u16)))
            .collect()
    }

    /// PPU VRAMを読み取り
    pub fn read_vram(&self) -> &[u8; 2048] {
        &self.cpu.bus.ppu.renderer.vram
    }

    /// PPU VRAMに書き込み
    pub fn write_vram(&mut self, address: u16, value: u8) {
        let addr = (address & 0x07FF) as usize;
        self.cpu.bus.ppu.renderer.vram[addr] = value;
    }

    /// PPU OAMを読み取り
    pub fn read_oam(&self) -> &[u8; 256] {
        &self.cpu.bus.ppu.renderer.oam
    }

    /// PPU OAMに書き込み
    pub fn write_oam(&mut self, address: u8, value: u8) {
        self.cpu.bus.ppu.renderer.oam[address as usize] = value;
    }

    /// PPU パレットを読み取り
    pub fn read_palette(&self) -> &[u8; 32] {
        &self.cpu.bus.ppu.renderer.palette
    }

    /// PPU パレットに書き込み
    pub fn write_palette(&mut self, address: u8, value: u8) {
        let addr = (address & 0x1F) as usize;
        self.cpu.bus.ppu.renderer.palette[addr] = value;
    }

    /// CHRメモリを読み取り
    pub fn read_chr(&self, address: u16) -> u8 {
        if let Some(ref c) = self.cpu.bus.cartridge {
            c.borrow().read_chr_byte(address)
        } else {
            0
        }
    }

    /// CHRメモリに書き込み（CHR RAMの場合のみ有効）
    pub fn write_chr(&mut self, address: u16, value: u8) {
        if let Some(ref c) = self.cpu.bus.cartridge {
            c.borrow_mut().write_chr_byte(address, value);
        }
    }

    /// PRG ROMを読み取り（生データ）
    pub fn read_prg_rom(&self) -> Option<Vec<u8>> {
        self.cpu.bus.cartridge.as_ref().map(|c| {
            let cart = c.borrow();
            // $8000から読み取り
            (0x8000u16..=0xFFFFu16)
                .map(|addr| cart.read_prg_byte(addr))
                .collect()
        })
    }

    /// メモリ検索（指定値を検索）
    pub fn search_memory(&self, value: u8) -> Vec<u16> {
        let mut results = Vec::new();

        // RAMを検索
        for (addr, &v) in self.cpu.bus.ram.iter().enumerate() {
            if v == value {
                results.push(addr as u16);
            }
        }

        results
    }

    /// メモリダンプを16進数文字列で取得
    pub fn hex_dump(&self, start: u16, length: usize) -> String {
        let memory = self.read_memory_range(start, length);
        memory_editor::MemoryEditor::hex_dump(&memory, 0, memory.len())
    }

    /// 逆アセンブル
    pub fn disassemble(&self, start: u16, count: usize) -> Vec<(u16, String)> {
        let memory = self.read_memory_range(start, count * 3); // 最大3バイト/命令
        memory_editor::MemoryEditor::disassemble(&memory, start, count)
    }

    /// 現在のPCから逆アセンブル
    pub fn disassemble_at_pc(&self, count: usize) -> Vec<(u16, String)> {
        self.disassemble(self.cpu.pc(), count)
    }

    /// スプライト情報を取得
    pub fn get_sprite_info(&self, index: u8) -> (u8, u8, u8, u8) {
        let base = (index as usize) * 4;
        let oam = &self.cpu.bus.ppu.renderer.oam;
        (oam[base], oam[base + 1], oam[base + 2], oam[base + 3])
    }

    /// 全スプライト情報を取得
    pub fn get_all_sprites(&self) -> Vec<(u8, u8, u8, u8, u8)> {
        (0..64)
            .map(|i| {
                let (y, tile, attr, x) = self.get_sprite_info(i);
                (i, y, tile, attr, x)
            })
            .collect()
    }
}

impl Default for Nes {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nes_creation() {
        let nes = Nes::new();
        assert_eq!(nes.cpu.pc(), 0);
    }
}
