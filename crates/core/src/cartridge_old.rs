//! # カートリッジ
//!
//! iNES形式のROMファイルを読み込み、マッパーを管理する。

use crate::error::{NesError, Result};

/// カートリッジ
pub struct Cartridge {
    /// PRG ROM（プログラムメモリ）
    prg_rom: Vec<u8>,
    /// CHR ROM（キャラクターメモリ、グラフィックデータ）
    chr_rom: Vec<u8>,
    /// マッパー番号
    mapper: u8,
    /// 縦/横ミラーリング（false=横、true=縦）
    vertical_mirroring: bool,
}

impl Cartridge {
    /// iNES形式のROMデータから作成
    pub fn from_ines(data: &[u8]) -> Result<Self> {
        if data.len() < 16 {
            return Err(NesError::InvalidRom("File too small".to_string()));
        }

        // iNESヘッダーのチェック
        if &data[0..4] != b"NES\x1A" {
            return Err(NesError::InvalidRom("Invalid iNES header".to_string()));
        }

        let prg_rom_size = data[4] as usize * 16384; // 16KB単位
        let chr_rom_size = data[5] as usize * 8192;  // 8KB単位
        let flags6 = data[6];
        let flags7 = data[7];

        let mapper = (flags7 & 0xF0) | (flags6 >> 4);
        let vertical_mirroring = (flags6 & 0x01) != 0;

        // トレーナーの有無（512バイト）
        let has_trainer = (flags6 & 0x04) != 0;
        let prg_start = 16 + if has_trainer { 512 } else { 0 };
        let chr_start = prg_start + prg_rom_size;

        if data.len() < chr_start + chr_rom_size {
            return Err(NesError::InvalidRom("File size mismatch".to_string()));
        }

        let prg_rom = data[prg_start..prg_start + prg_rom_size].to_vec();
        let chr_rom = data[chr_start..chr_start + chr_rom_size].to_vec();

        log::info!(
            "Loaded ROM: PRG={} KB, CHR={} KB, Mapper={}, Mirroring={}",
            prg_rom_size / 1024,
            chr_rom_size / 1024,
            mapper,
            if vertical_mirroring { "Vertical" } else { "Horizontal" }
        );

        Ok(Self {
            prg_rom,
            chr_rom,
            mapper,
            vertical_mirroring,
        })
    }

    /// CPU側のメモリ読み込み
    pub fn cpu_read(&mut self, addr: u16) -> u8 {
        match self.mapper {
            0 => self.mapper0_cpu_read(addr),
            _ => {
                log::warn!("Unsupported mapper: {}", self.mapper);
                0
            }
        }
    }

    /// CPU側のメモリ書き込み
    pub fn cpu_write(&mut self, addr: u16, _data: u8) {
        match self.mapper {
            0 => {
                // Mapper 0 は PRG-ROM なので書き込み不可
                log::trace!("Attempted write to PRG-ROM at {:#06x}", addr);
            }
            _ => {
                log::warn!("Unsupported mapper: {}", self.mapper);
            }
        }
    }

    /// PPU側のメモリ読み込み
    pub fn ppu_read(&mut self, addr: u16) -> u8 {
        match self.mapper {
            0 => self.mapper0_ppu_read(addr),
            _ => {
                log::warn!("Unsupported mapper: {}", self.mapper);
                0
            }
        }
    }

    /// PPU側のメモリ書き込み
    pub fn ppu_write(&mut self, addr: u16, data: u8) {
        match self.mapper {
            0 => self.mapper0_ppu_write(addr, data),
            _ => {
                log::warn!("Unsupported mapper: {}", self.mapper);
            }
        }
    }

    // Mapper 0 (NROM) の実装
    fn mapper0_cpu_read(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xFFFF => {
                let index = if self.prg_rom.len() == 16384 {
                    // 16KB ROM の場合はミラーリング
                    (addr & 0x3FFF) as usize
                } else {
                    // 32KB ROM
                    (addr & 0x7FFF) as usize
                };
                self.prg_rom.get(index).copied().unwrap_or(0)
            }
            _ => 0,
        }
    }

    fn mapper0_ppu_read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => {
                let index = (addr & 0x1FFF) as usize;
                self.chr_rom.get(index).copied().unwrap_or(0)
            }
            _ => 0,
        }
    }

    fn mapper0_ppu_write(&mut self, addr: u16, _data: u8) {
        // Mapper 0 は CHR-ROM なので書き込み不可
        log::trace!("Attempted write to CHR-ROM at {:#06x}", addr);
    }

    pub fn mapper(&self) -> u8 {
        self.mapper
    }

    pub fn vertical_mirroring(&self) -> bool {
        self.vertical_mirroring
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_rom() {
        let data = vec![0; 10];
        assert!(Cartridge::from_ines(&data).is_err());
    }

    #[test]
    fn test_valid_ines_header() {
        let mut data = vec![0; 16 + 16384 + 8192]; // Header + 16KB PRG + 8KB CHR
        data[0..4].copy_from_slice(b"NES\x1A");
        data[4] = 1; // 1 * 16KB PRG
        data[5] = 1; // 1 * 8KB CHR
        data[6] = 0; // Mapper 0, horizontal mirroring

        let cartridge = Cartridge::from_ines(&data).unwrap();
        assert_eq!(cartridge.mapper, 0);
        assert!(!cartridge.vertical_mirroring);
    }
}
