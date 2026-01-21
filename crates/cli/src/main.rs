//! # NES CLI
//!
//! SDL2を使用したNESエミュレータのデスクトップ版フロントエンド

use anyhow::Result;
use clap::Parser;
use nes_core::controller::Button;
use nes_core::Nes;
use sdl2::audio::{AudioCallback, AudioSpecDesired};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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

    /// オーディオを無効化
    #[arg(long)]
    no_audio: bool,
}

struct AudioPlayer {
    samples: Arc<Mutex<Vec<f32>>>,
    position: usize,
}

impl AudioCallback for AudioPlayer {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        let mut samples = self.samples.lock().unwrap();
        for sample in out.iter_mut() {
            if self.position < samples.len() {
                *sample = samples[self.position];
                self.position += 1;
            } else {
                *sample = 0.0;
            }
        }
        // Drain consumed samples
        if self.position >= samples.len() {
            samples.clear();
            self.position = 0;
        } else if self.position > 0 {
            samples.drain(0..self.position);
            self.position = 0;
        }
    }
}

fn keycode_to_button(keycode: Keycode) -> Option<Button> {
    match keycode {
        Keycode::Up => Some(Button::Up),
        Keycode::Down => Some(Button::Down),
        Keycode::Left => Some(Button::Left),
        Keycode::Right => Some(Button::Right),
        Keycode::A => Some(Button::A),
        Keycode::S => Some(Button::B),
        Keycode::D => Some(Button::Select),
        Keycode::F => Some(Button::Start),
        _ => None,
    }
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

    // オーディオ初期化
    let audio_samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let _audio_device = if !args.no_audio {
        let audio_subsystem = sdl_context.audio().map_err(|e| anyhow::anyhow!(e))?;
        let desired_spec = AudioSpecDesired {
            freq: Some(44100),
            channels: Some(1),
            samples: Some(1024),
        };
        let device = audio_subsystem
            .open_playback(None, &desired_spec, |_spec| AudioPlayer {
                samples: audio_samples.clone(),
                position: 0,
            })
            .map_err(|e| anyhow::anyhow!(e))?;
        device.resume();
        log::info!("Audio initialized");
        Some(device)
    } else {
        log::info!("Audio disabled");
        None
    };

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
                    if let Some(button) = keycode_to_button(keycode) {
                        nes.cpu_state_mut().bus.controller.press(button);
                    }
                }
                Event::KeyUp {
                    keycode: Some(keycode),
                    ..
                } => {
                    if let Some(button) = keycode_to_button(keycode) {
                        nes.cpu_state_mut().bus.controller.release(button);
                    }
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

                // オーディオサンプルを取得してバッファに追加
                if !args.no_audio {
                    let samples = nes.get_audio_samples();
                    if !samples.is_empty() {
                        let mut audio_buffer = audio_samples.lock().unwrap();
                        audio_buffer.extend(samples);
                    }
                }
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
