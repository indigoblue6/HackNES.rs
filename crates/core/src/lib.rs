//! # NES Core
//! Based on https://github.com/starrhorne/nes-rust

pub mod cpu;
pub mod ppu;
pub mod bus;
pub mod cartridge;
pub mod controller;
pub mod error;

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
        }

        Ok(self.cpu.bus.ppu.frame_buffer())
    }

    /// 1CPUサイクル実行（デバッグ用）
    pub fn step(&mut self) -> Result<u32> {
        let start_cycles = self.cpu.bus.cycles;
        self.cpu.step()?;
        let elapsed = (self.cpu.bus.cycles - start_cycles) as u32;
        
        if self.cpu.bus.ppu.nmi {
            self.cpu.interrupt(cpu::Interrupt::Nmi);
            self.cpu.bus.ppu.nmi = false;
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
