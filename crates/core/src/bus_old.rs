//! # メモリバス
//!
//! NESのメモリマップを抽象化し、CPU/PPU/カートリッジ間の通信を管理する。

use crate::cartridge::Cartridge;
use crate::ppu::Ppu;

/// NESのメモリバス
pub struct Bus {
    /// 内部RAM (2KB, $0000-$07FF, $0800-$1FFF はミラー)
    ram: [u8; 0x0800],
    /// カートリッジ
    cartridge: Option<Cartridge>,
    /// PPUへの参照（レジスタアクセス用）
    ppu: Option<*mut Ppu>,
}

impl Bus {
    /// 新しいBusインスタンスを作成
    pub fn new() -> Self {
        Self {
            ram: [0; 0x0800],
            cartridge: None,
            ppu: None,
        }
    }
    
    /// PPUへの参照を設定
    pub fn set_ppu(&mut self, ppu: *mut Ppu) {
        self.ppu = Some(ppu);
    }

    /// カートリッジを挿入
    pub fn insert_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
    }

    /// CPU側のメモリ読み込み
    /// 
    /// # メモリマップ
    /// - $0000-$07FF: 内部RAM
    /// - $0800-$1FFF: RAMのミラー
    /// - $2000-$2007: PPUレジスタ
    /// - $2008-$3FFF: PPUレジスタのミラー
    /// - $4000-$4017: APU、I/Oレジスタ
    /// - $4018-$401F: APU、I/Oテスト用（通常未使用）
    /// - $4020-$FFFF: カートリッジ空間
    pub fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            // 内部RAM
            0x0000..=0x1FFF => {
                let mirrored_addr = addr & 0x07FF;
                self.ram[mirrored_addr as usize]
            }
            // PPUレジスタ
            0x2000..=0x3FFF => {
                let ppu_addr = 0x2000 + (addr & 0x0007);
                if let Some(ppu_ptr) = self.ppu {
                    unsafe {
                        (*ppu_ptr).read_register(ppu_addr)
                    }
                } else {
                    log::trace!("PPU register read at {:#06x}", addr);
                    0
                }
            }
            // APU、I/Oレジスタ（未実装）
            0x4000..=0x4017 => {
                log::trace!("APU/IO register read at {:#06x}", addr);
                0
            }
            // カートリッジ空間
            0x4020..=0xFFFF => {
                if let Some(ref mut cartridge) = self.cartridge {
                    cartridge.cpu_read(addr)
                } else {
                    log::warn!("No cartridge inserted, read at {:#06x}", addr);
                    0
                }
            }
            _ => 0,
        }
    }

    /// CPU側のメモリ書き込み
    pub fn cpu_write(&mut self, addr: u16, data: u8) {
        match addr {
            // 内部RAM
            0x0000..=0x1FFF => {
                let mirrored_addr = addr & 0x07FF;
                self.ram[mirrored_addr as usize] = data;
            }
            // PPUレジスタ
            0x2000..=0x3FFF => {
                let ppu_addr = 0x2000 + (addr & 0x0007);
                if let Some(ppu_ptr) = self.ppu {
                    unsafe {
                        (*ppu_ptr).write_register(ppu_addr, data);
                    }
                } else {
                    log::trace!("PPU register write at {:#06x}: {:#04x}", addr, data);
                }
            }
            // APU、I/Oレジスタ（未実装）
            0x4000..=0x4017 => {
                log::trace!("APU/IO register write at {:#06x}: {:#04x}", addr, data);
            }
            // カートリッジ空間
            0x4020..=0xFFFF => {
                if let Some(ref mut cartridge) = self.cartridge {
                    cartridge.cpu_write(addr, data);
                } else {
                    log::warn!("No cartridge inserted, write at {:#06x}", addr);
                }
            }
            _ => {}
        }
    }

    /// PPU側のメモリ読み込み（将来実装）
    pub fn ppu_read(&mut self, addr: u16) -> u8 {
        if let Some(ref mut cartridge) = self.cartridge {
            cartridge.ppu_read(addr)
        } else {
            0
        }
    }

    /// PPU側のメモリ書き込み（将来実装）
    pub fn ppu_write(&mut self, addr: u16, data: u8) {
        if let Some(ref mut cartridge) = self.cartridge {
            cartridge.ppu_write(addr, data);
        }
    }
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ram_access() {
        let mut bus = Bus::new();
        bus.cpu_write(0x0000, 0x42);
        assert_eq!(bus.cpu_read(0x0000), 0x42);
    }

    #[test]
    fn test_ram_mirroring() {
        let mut bus = Bus::new();
        bus.cpu_write(0x0000, 0x42);
        assert_eq!(bus.cpu_read(0x0800), 0x42);
        assert_eq!(bus.cpu_read(0x1000), 0x42);
        assert_eq!(bus.cpu_read(0x1800), 0x42);
    }
}
