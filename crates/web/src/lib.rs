//! # NES Web
//!
//! WebAssembly対応のNESエミュレータ

use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;
use web_sys::{CanvasRenderingContext2d, ImageData};

use nes_core::Nes;
use nes_core::controller::Button;

/// WebAssembly用のNESエミュレータラッパー
#[wasm_bindgen]
pub struct NesWeb {
    nes: Nes,
}

#[wasm_bindgen]
impl NesWeb {
    /// 新しいNESインスタンスを作成
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        // パニック時のスタックトレースを有効化
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        // ログ初期化（エラーは無視）
        let _ = std::panic::catch_unwind(|| {
            wasm_logger::init(wasm_logger::Config::default());
        });

        log::info!("NES Web initialized");

        Self { nes: Nes::new() }
    }

    /// ROMをロード
    pub fn load_rom(&mut self, rom_data: &[u8]) -> Result<(), JsValue> {
        self.nes
            .load_rom(rom_data)
            .map_err(|e| JsValue::from_str(&format!("Failed to load ROM: {}", e)))
    }

    /// システムをリセット
    pub fn reset(&mut self) {
        self.nes.reset();
    }

    /// 1フレーム実行
    pub fn step_frame(&mut self) -> Result<(), JsValue> {
        self.nes
            .step_frame()
            .map_err(|e| JsValue::from_str(&format!("Emulation error: {}", e)))?;
        Ok(())
    }

    /// フレームバッファを取得してCanvasに描画
    pub fn render(&self, ctx: &CanvasRenderingContext2d) -> Result<(), JsValue> {
        let frame_buffer = self.nes.ppu_state().frame_buffer();

        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(frame_buffer),
            256,
            240,
        )?;

        ctx.put_image_data(&image_data, 0.0, 0.0)?;
        Ok(())
    }

    /// CPU状態の取得（デバッグ用）
    pub fn get_cpu_state(&self) -> String {
        let cpu = self.nes.cpu_state();
        format!(
            "PC: {:#06x}, SP: {:#04x}, A: {:#04x}, X: {:#04x}, Y: {:#04x}, Status: {:#010b}",
            cpu.pc(),
            cpu.sp(),
            cpu.a,
            cpu.x,
            cpu.y,
            cpu.status()
        )
    }

    /// PPU状態の取得（デバッグ用）
    pub fn get_ppu_state(&self) -> String {
        let ppu = self.nes.ppu_state();
        format!(
            "Scanline: {}, Cycle: {}, Frame: {}",
            ppu.scanline(),
            ppu.cycle(),
            ppu.frame()
        )
    }

    /// オーディオサンプルを取得
    pub fn get_audio_samples(&mut self) -> Vec<f32> {
        self.nes.get_audio_samples()
    }

    /// オーディオサンプル数を取得
    pub fn get_audio_sample_count(&self) -> usize {
        // APU sample buffer is drained by get_audio_samples, return expected samples per frame
        (44100.0 / 60.0) as usize
    }

    /// キーボード入力を処理（キーダウン）
    /// キーコード: ArrowUp, ArrowDown, ArrowLeft, ArrowRight, a, s, d, f
    pub fn key_down(&mut self, key: &str) {
        if let Some(button) = self.key_to_button(key) {
            self.nes.cpu_state_mut().bus.controller.press(button);
        }
    }

    /// キーボード入力を処理（キーアップ）
    pub fn key_up(&mut self, key: &str) {
        if let Some(button) = self.key_to_button(key) {
            self.nes.cpu_state_mut().bus.controller.release(button);
        }
    }

    fn key_to_button(&self, key: &str) -> Option<Button> {
        match key {
            "ArrowUp" => Some(Button::Up),
            "ArrowDown" => Some(Button::Down),
            "ArrowLeft" => Some(Button::Left),
            "ArrowRight" => Some(Button::Right),
            "a" | "A" => Some(Button::A),
            "s" | "S" => Some(Button::B),
            "d" | "D" => Some(Button::Select),
            "f" | "F" => Some(Button::Start),
            _ => None,
        }
    }

    // ========== メモリエディタ API ==========

    /// RAMを読み取り (2048バイト)
    pub fn read_ram(&self) -> Vec<u8> {
        self.nes.read_ram().to_vec()
    }

    /// RAMの特定アドレスを読み取り
    pub fn peek_ram(&self, address: u16) -> u8 {
        self.nes.read_ram()[(address & 0x07FF) as usize]
    }

    /// RAMに書き込み
    pub fn poke_ram(&mut self, address: u16, value: u8) {
        self.nes.write_ram(address, value);
    }

    /// 任意のメモリアドレスを読み取り
    pub fn peek_memory(&self, address: u16) -> u8 {
        self.nes.peek_memory(address)
    }

    /// 任意のメモリアドレスに書き込み
    pub fn poke_memory(&mut self, address: u16, value: u8) {
        self.nes.poke_memory(address, value);
    }

    /// メモリ範囲を読み取り
    pub fn read_memory_range(&self, start: u16, length: usize) -> Vec<u8> {
        self.nes.read_memory_range(start, length)
    }

    /// VRAMを読み取り
    pub fn read_vram(&self) -> Vec<u8> {
        self.nes.read_vram().to_vec()
    }

    /// VRAMに書き込み
    pub fn poke_vram(&mut self, address: u16, value: u8) {
        self.nes.write_vram(address, value);
    }

    /// OAMを読み取り
    pub fn read_oam(&self) -> Vec<u8> {
        self.nes.read_oam().to_vec()
    }

    /// OAMに書き込み
    pub fn poke_oam(&mut self, address: u8, value: u8) {
        self.nes.write_oam(address, value);
    }

    /// パレットを読み取り
    pub fn read_palette(&self) -> Vec<u8> {
        self.nes.read_palette().to_vec()
    }

    /// パレットに書き込み
    pub fn poke_palette(&mut self, address: u8, value: u8) {
        self.nes.write_palette(address, value);
    }

    /// CHRを読み取り
    pub fn peek_chr(&self, address: u16) -> u8 {
        self.nes.read_chr(address)
    }

    /// CHRに書き込み
    pub fn poke_chr(&mut self, address: u16, value: u8) {
        self.nes.write_chr(address, value);
    }

    /// 値を検索
    pub fn search_value(&self, value: u8) -> Vec<u16> {
        self.nes.search_memory(value)
    }

    /// メモリダンプ（16進数文字列）
    pub fn hex_dump(&self, start: u16, length: usize) -> String {
        self.nes.hex_dump(start, length)
    }

    /// 逆アセンブル
    pub fn disassemble(&self, start: u16, count: usize) -> String {
        self.nes
            .disassemble(start, count)
            .iter()
            .map(|(addr, inst)| format!("{:04X}: {}", addr, inst))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 現在のPCから逆アセンブル
    pub fn disassemble_at_pc(&self, count: usize) -> String {
        self.nes
            .disassemble_at_pc(count)
            .iter()
            .map(|(addr, inst)| format!("{:04X}: {}", addr, inst))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// スプライト情報を取得（JSON形式）
    pub fn get_sprites_json(&self) -> String {
        let sprites = self.nes.get_all_sprites();
        let json: Vec<String> = sprites
            .iter()
            .filter(|(_, y, _, _, _)| *y < 0xEF) // 非表示スプライトを除外
            .map(|(idx, y, tile, attr, x)| {
                format!(
                    r#"{{"index":{},"x":{},"y":{},"tile":{},"attr":{}}}"#,
                    idx, x, y, tile, attr
                )
            })
            .collect();
        format!("[{}]", json.join(","))
    }

    /// Game Genieコードを適用
    pub fn apply_game_genie(&mut self, code: &str) -> Result<String, JsValue> {
        use nes_core::memory_editor::CheatCode;

        match CheatCode::from_game_genie(code) {
            Some(cheat) => {
                self.nes.poke_memory(cheat.address, cheat.value);
                Ok(format!(
                    "Applied: {:04X} = {:02X}",
                    cheat.address, cheat.value
                ))
            }
            None => Err(JsValue::from_str("Invalid Game Genie code")),
        }
    }

    /// RAWチートコードを適用 (AAAA:VV形式)
    pub fn apply_raw_cheat(&mut self, code: &str) -> Result<String, JsValue> {
        use nes_core::memory_editor::CheatCode;

        match CheatCode::from_raw(code) {
            Some(cheat) => {
                self.nes.poke_memory(cheat.address, cheat.value);
                Ok(format!(
                    "Applied: {:04X} = {:02X}",
                    cheat.address, cheat.value
                ))
            }
            None => Err(JsValue::from_str("Invalid cheat code format (use AAAA:VV)")),
        }
    }
}

/// JavaScriptのコンソールにログを出力（初期化）
#[wasm_bindgen]
pub fn init_logger() {
    let _ = std::panic::catch_unwind(|| {
        wasm_logger::init(wasm_logger::Config::default());
    });
}
