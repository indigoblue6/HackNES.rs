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
}

/// JavaScriptのコンソールにログを出力（初期化）
#[wasm_bindgen]
pub fn init_logger() {
    let _ = std::panic::catch_unwind(|| {
        wasm_logger::init(wasm_logger::Config::default());
    });
}
