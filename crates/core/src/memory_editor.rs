//! # Memory Editor
//! メモリの読み書き、検索、監視機能を提供

use std::collections::HashMap;

/// メモリ領域の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryRegion {
    /// CPU RAM ($0000-$07FF, 2KB mirrored to $1FFF)
    Ram,
    /// PPU VRAM (2KB nametable)
    Vram,
    /// PPU OAM (256 bytes)
    Oam,
    /// PPU Palette (32 bytes)
    Palette,
    /// Cartridge PRG ROM
    PrgRom,
    /// Cartridge PRG RAM ($6000-$7FFF)
    PrgRam,
    /// Cartridge CHR (ROM or RAM)
    Chr,
}

/// 検索条件
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchCondition {
    /// 指定値と等しい
    Equal(u8),
    /// 指定値と等しくない
    NotEqual(u8),
    /// 指定値より大きい
    GreaterThan(u8),
    /// 指定値より小さい
    LessThan(u8),
    /// 指定範囲内
    Between(u8, u8),
    /// 前回値から増加
    Increased,
    /// 前回値から減少
    Decreased,
    /// 前回値と同じ
    Unchanged,
    /// 前回値と異なる
    Changed,
}

/// 検索結果
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub address: u16,
    pub value: u8,
    pub previous_value: Option<u8>,
}

/// ウォッチポイント
#[derive(Debug, Clone)]
pub struct Watchpoint {
    pub id: u32,
    pub region: MemoryRegion,
    pub address: u16,
    pub label: String,
    pub last_value: u8,
}

/// メモリエディタ
pub struct MemoryEditor {
    /// アクティブな検索のスナップショット
    search_snapshot: Option<Vec<u8>>,
    /// 検索対象領域
    search_region: MemoryRegion,
    /// 現在の検索結果
    search_results: Vec<SearchResult>,
    /// ウォッチポイント
    watchpoints: HashMap<u32, Watchpoint>,
    /// 次のウォッチポイントID
    next_watchpoint_id: u32,
}

impl MemoryEditor {
    pub fn new() -> Self {
        Self {
            search_snapshot: None,
            search_region: MemoryRegion::Ram,
            search_results: Vec::new(),
            watchpoints: HashMap::new(),
            next_watchpoint_id: 1,
        }
    }

    /// 検索を開始（現在のメモリ状態をスナップショット）
    pub fn start_search(&mut self, region: MemoryRegion, memory: &[u8]) {
        self.search_snapshot = Some(memory.to_vec());
        self.search_region = region;
        self.search_results.clear();
    }

    /// 検索を実行
    pub fn search(&mut self, condition: SearchCondition, current_memory: &[u8]) -> &[SearchResult] {
        self.search_results.clear();

        let snapshot = self.search_snapshot.as_ref();

        for (addr, &value) in current_memory.iter().enumerate() {
            let prev = snapshot.and_then(|s| s.get(addr).copied());

            let matches = match condition {
                SearchCondition::Equal(v) => value == v,
                SearchCondition::NotEqual(v) => value != v,
                SearchCondition::GreaterThan(v) => value > v,
                SearchCondition::LessThan(v) => value < v,
                SearchCondition::Between(lo, hi) => value >= lo && value <= hi,
                SearchCondition::Increased => prev.map_or(false, |p| value > p),
                SearchCondition::Decreased => prev.map_or(false, |p| value < p),
                SearchCondition::Unchanged => prev.map_or(false, |p| value == p),
                SearchCondition::Changed => prev.map_or(false, |p| value != p),
            };

            if matches {
                self.search_results.push(SearchResult {
                    address: addr as u16,
                    value,
                    previous_value: prev,
                });
            }
        }

        // 現在のメモリを次回比較用にスナップショット
        self.search_snapshot = Some(current_memory.to_vec());

        &self.search_results
    }

    /// フィルタ付き検索（既存の結果を絞り込み）
    pub fn filter_search(&mut self, condition: SearchCondition, current_memory: &[u8]) -> &[SearchResult] {
        let snapshot = self.search_snapshot.as_ref();

        self.search_results.retain(|result| {
            let addr = result.address as usize;
            let value = current_memory.get(addr).copied().unwrap_or(0);
            let prev = snapshot.and_then(|s| s.get(addr).copied());

            match condition {
                SearchCondition::Equal(v) => value == v,
                SearchCondition::NotEqual(v) => value != v,
                SearchCondition::GreaterThan(v) => value > v,
                SearchCondition::LessThan(v) => value < v,
                SearchCondition::Between(lo, hi) => value >= lo && value <= hi,
                SearchCondition::Increased => prev.map_or(false, |p| value > p),
                SearchCondition::Decreased => prev.map_or(false, |p| value < p),
                SearchCondition::Unchanged => prev.map_or(false, |p| value == p),
                SearchCondition::Changed => prev.map_or(false, |p| value != p),
            }
        });

        // 値を更新
        for result in &mut self.search_results {
            let addr = result.address as usize;
            result.previous_value = Some(result.value);
            result.value = current_memory.get(addr).copied().unwrap_or(0);
        }

        self.search_snapshot = Some(current_memory.to_vec());

        &self.search_results
    }

    /// 検索をリセット
    pub fn reset_search(&mut self) {
        self.search_snapshot = None;
        self.search_results.clear();
    }

    /// 検索結果を取得
    pub fn get_search_results(&self) -> &[SearchResult] {
        &self.search_results
    }

    /// ウォッチポイントを追加
    pub fn add_watchpoint(&mut self, region: MemoryRegion, address: u16, label: &str) -> u32 {
        let id = self.next_watchpoint_id;
        self.next_watchpoint_id += 1;

        self.watchpoints.insert(id, Watchpoint {
            id,
            region,
            address,
            label: label.to_string(),
            last_value: 0,
        });

        id
    }

    /// ウォッチポイントを削除
    pub fn remove_watchpoint(&mut self, id: u32) -> bool {
        self.watchpoints.remove(&id).is_some()
    }

    /// ウォッチポイントをすべて取得
    pub fn get_watchpoints(&self) -> Vec<&Watchpoint> {
        self.watchpoints.values().collect()
    }

    /// ウォッチポイントの値を更新
    pub fn update_watchpoint(&mut self, id: u32, value: u8) {
        if let Some(wp) = self.watchpoints.get_mut(&id) {
            wp.last_value = value;
        }
    }

    /// メモリダンプを16進数文字列で取得
    pub fn hex_dump(memory: &[u8], start: usize, length: usize) -> String {
        let mut result = String::new();
        let end = (start + length).min(memory.len());

        for addr in (start..end).step_by(16) {
            // アドレス
            result.push_str(&format!("{:04X}: ", addr));

            // 16進ダンプ
            for i in 0..16 {
                if addr + i < end {
                    result.push_str(&format!("{:02X} ", memory[addr + i]));
                } else {
                    result.push_str("   ");
                }
                if i == 7 {
                    result.push(' ');
                }
            }

            // ASCII表示
            result.push_str(" |");
            for i in 0..16 {
                if addr + i < end {
                    let b = memory[addr + i];
                    if b.is_ascii_graphic() || b == b' ' {
                        result.push(b as char);
                    } else {
                        result.push('.');
                    }
                }
            }
            result.push_str("|\n");
        }

        result
    }

    /// 逆アセンブル（簡易版）
    pub fn disassemble(memory: &[u8], start: u16, count: usize) -> Vec<(u16, String)> {
        let mut result = Vec::new();
        let mut pc = start as usize;

        for _ in 0..count {
            if pc >= memory.len() {
                break;
            }

            let opcode = memory[pc];
            let (mnemonic, size) = Self::decode_instruction(opcode);

            let operand = match size {
                1 => String::new(),
                2 => {
                    if pc + 1 < memory.len() {
                        format!(" ${:02X}", memory[pc + 1])
                    } else {
                        " ??".to_string()
                    }
                }
                3 => {
                    if pc + 2 < memory.len() {
                        format!(" ${:02X}{:02X}", memory[pc + 2], memory[pc + 1])
                    } else {
                        " ????".to_string()
                    }
                }
                _ => String::new(),
            };

            result.push((pc as u16, format!("{}{}", mnemonic, operand)));
            pc += size;
        }

        result
    }

    fn decode_instruction(opcode: u8) -> (&'static str, usize) {
        match opcode {
            // Load/Store
            0xA9 => ("LDA #", 2),
            0xA5 => ("LDA", 2),
            0xAD => ("LDA", 3),
            0xA2 => ("LDX #", 2),
            0xA6 => ("LDX", 2),
            0xAE => ("LDX", 3),
            0xA0 => ("LDY #", 2),
            0xA4 => ("LDY", 2),
            0xAC => ("LDY", 3),
            0x85 => ("STA", 2),
            0x8D => ("STA", 3),
            0x86 => ("STX", 2),
            0x8E => ("STX", 3),
            0x84 => ("STY", 2),
            0x8C => ("STY", 3),
            // Arithmetic
            0x69 => ("ADC #", 2),
            0x65 => ("ADC", 2),
            0x6D => ("ADC", 3),
            0xE9 => ("SBC #", 2),
            0xE5 => ("SBC", 2),
            0xED => ("SBC", 3),
            // Compare
            0xC9 => ("CMP #", 2),
            0xC5 => ("CMP", 2),
            0xCD => ("CMP", 3),
            0xE0 => ("CPX #", 2),
            0xC0 => ("CPY #", 2),
            // Logic
            0x29 => ("AND #", 2),
            0x09 => ("ORA #", 2),
            0x49 => ("EOR #", 2),
            // Shift
            0x0A => ("ASL A", 1),
            0x4A => ("LSR A", 1),
            0x2A => ("ROL A", 1),
            0x6A => ("ROR A", 1),
            // Inc/Dec
            0xE6 => ("INC", 2),
            0xEE => ("INC", 3),
            0xC6 => ("DEC", 2),
            0xCE => ("DEC", 3),
            0xE8 => ("INX", 1),
            0xCA => ("DEX", 1),
            0xC8 => ("INY", 1),
            0x88 => ("DEY", 1),
            // Transfer
            0xAA => ("TAX", 1),
            0xA8 => ("TAY", 1),
            0x8A => ("TXA", 1),
            0x98 => ("TYA", 1),
            0x9A => ("TXS", 1),
            0xBA => ("TSX", 1),
            // Stack
            0x48 => ("PHA", 1),
            0x68 => ("PLA", 1),
            0x08 => ("PHP", 1),
            0x28 => ("PLP", 1),
            // Branch
            0x10 => ("BPL", 2),
            0x30 => ("BMI", 2),
            0x50 => ("BVC", 2),
            0x70 => ("BVS", 2),
            0x90 => ("BCC", 2),
            0xB0 => ("BCS", 2),
            0xD0 => ("BNE", 2),
            0xF0 => ("BEQ", 2),
            // Jump
            0x4C => ("JMP", 3),
            0x6C => ("JMP (", 3),
            0x20 => ("JSR", 3),
            0x60 => ("RTS", 1),
            0x40 => ("RTI", 1),
            // Flags
            0x18 => ("CLC", 1),
            0x38 => ("SEC", 1),
            0x58 => ("CLI", 1),
            0x78 => ("SEI", 1),
            0xB8 => ("CLV", 1),
            0xD8 => ("CLD", 1),
            0xF8 => ("SED", 1),
            // Other
            0xEA => ("NOP", 1),
            0x00 => ("BRK", 1),
            0x24 => ("BIT", 2),
            0x2C => ("BIT", 3),
            _ => ("???", 1),
        }
    }
}

impl Default for MemoryEditor {
    fn default() -> Self {
        Self::new()
    }
}

/// チートコード (Game Genie形式)
#[derive(Debug, Clone)]
pub struct CheatCode {
    pub address: u16,
    pub value: u8,
    pub compare: Option<u8>,
    pub enabled: bool,
    pub description: String,
}

impl CheatCode {
    /// Game Genie コードをデコード
    pub fn from_game_genie(code: &str) -> Option<Self> {
        let code = code.to_uppercase().replace("-", "");
        if code.len() != 6 && code.len() != 8 {
            return None;
        }

        let decode_char = |c: char| -> Option<u8> {
            match c {
                'A' => Some(0x0),
                'P' => Some(0x1),
                'Z' => Some(0x2),
                'L' => Some(0x3),
                'G' => Some(0x4),
                'I' => Some(0x5),
                'T' => Some(0x6),
                'Y' => Some(0x7),
                'E' => Some(0x8),
                'O' => Some(0x9),
                'X' => Some(0xA),
                'U' => Some(0xB),
                'K' => Some(0xC),
                'S' => Some(0xD),
                'V' => Some(0xE),
                'N' => Some(0xF),
                _ => None,
            }
        };

        let chars: Vec<u8> = code.chars().filter_map(decode_char).collect();
        if chars.len() != code.len() {
            return None;
        }

        // 6文字コード: AAAA-VV
        // 8文字コード: AAAA-VV-CC
        let address = 0x8000
            | ((chars[3] & 0x7) as u16) << 12
            | ((chars[5] & 0x7) as u16) << 8
            | ((chars[4] & 0x8) as u16) << 8
            | ((chars[2] & 0x7) as u16) << 4
            | ((chars[1] & 0x8) as u16) << 4
            | (chars[4] & 0x7) as u16
            | ((chars[3] & 0x8) as u16);

        let value = ((chars[1] & 0x7) << 4)
            | ((chars[0] & 0x8))
            | (chars[0] & 0x7)
            | ((chars[5] & 0x8));

        let compare = if code.len() == 8 {
            Some(
                ((chars[7] & 0x7) << 4)
                    | ((chars[6] & 0x8))
                    | (chars[6] & 0x7)
                    | ((chars[7] & 0x8)),
            )
        } else {
            None
        };

        Some(CheatCode {
            address,
            value,
            compare,
            enabled: true,
            description: code,
        })
    }

    /// Pro Action Replay コードをデコード (AAAA:VV形式)
    pub fn from_raw(code: &str) -> Option<Self> {
        let parts: Vec<&str> = code.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let address = u16::from_str_radix(parts[0], 16).ok()?;
        let value = u8::from_str_radix(parts[1], 16).ok()?;

        Some(CheatCode {
            address,
            value,
            compare: None,
            enabled: true,
            description: code.to_string(),
        })
    }
}
