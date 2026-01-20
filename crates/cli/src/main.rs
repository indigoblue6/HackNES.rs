//! # NES CLI
//!
//! SDL2を使用したNESエミュレータのデスクトップ版フロントエンド

use anyhow::Result;
use clap::Parser;
use nes_core::Nes;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use std::path::PathBuf;

/// NESエミュレータ CLI
#[derive(Parser, Debug)]
#[command(name = "HackNES.rs")]
#[command(about = "NES Emulator with visualization features", long_about = None)]
struct Args {
    /// ROMファイルのパス
    #[arg(value_name = "ROM")]
    rom_path: PathBuf,

    /// スケールファクタ（デフォルト: 3）
    #[arg(short, long, default_value = "3")]
    scale: u32,

    /// デバッグモード
    #[arg(short, long)]
    debug: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    // ROMの読み込み
    let rom_data = std::fs::read(&args.rom_path)?;
    log::info!("Loaded ROM: {:?}", args.rom_path);

    // NESの初期化
    let mut nes = Nes::new();
    nes.load_rom(&rom_data)?;

    // SDL2の初期化
    let sdl_context = sdl2::init().map_err(|e| anyhow::anyhow!(e))?;
    let video_subsystem = sdl_context.video().map_err(|e| anyhow::anyhow!(e))?;

    let window_width = 256 * args.scale;
    let window_height = 240 * args.scale;

    let window = video_subsystem
        .window("HackNES.rs", window_width, window_height)
        .position_centered()
        .build()?;

    let mut canvas = window.into_canvas().build()?;
    let texture_creator = canvas.texture_creator();

    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGBA32, 256, 240)
        .map_err(|e| anyhow::anyhow!(e))?;

    let mut event_pump = sdl_context.event_pump().map_err(|e| anyhow::anyhow!(e))?;

    log::info!("Starting emulation...");

    'running: loop {
        // イベント処理
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => {
                    if args.debug {
                        log::debug!("Key pressed: {:?}", keycode);
                    }
                    // TODO: コントローラー入力処理
                }
                Event::KeyUp {
                    keycode: Some(_keycode),
                    ..
                } => {
                    // TODO: コントローラー入力処理
                }
                _ => {}
            }
        }

        // 1フレーム実行
        match nes.step_frame() {
            Ok(frame_buffer) => {
                // フレームバッファをテクスチャに転送
                texture
                    .update(None, frame_buffer, 256 * 4)
                    .map_err(|e| anyhow::anyhow!(e))?;

                canvas.clear();
                canvas
                    .copy(&texture, None, None)
                    .map_err(|e| anyhow::anyhow!(e))?;
                canvas.present();
            }
            Err(e) => {
                log::error!("Emulation error: {}", e);
                if args.debug {
                    break 'running;
                }
            }
        }

        // フレームレート制限（約60 FPS）
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    log::info!("Emulation stopped");
    Ok(())
}
