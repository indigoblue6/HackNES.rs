//! # PPU (Picture Processing Unit)
//!
//! NESの描画を担当するチップ。256x240ピクセルの画面を生成。

use crate::bus::Bus;

/// PPU の解像度
pub const SCREEN_WIDTH: usize = 256;
pub const SCREEN_HEIGHT: usize = 240;

/// NESのパレット（簡易版）
const PALETTE: [[u8; 3]; 64] = [
    [0x80, 0x80, 0x80], [0x00, 0x3D, 0xA6], [0x00, 0x12, 0xB0], [0x44, 0x00, 0x96],
    [0xA1, 0x00, 0x5E], [0xC7, 0x00, 0x28], [0xBA, 0x06, 0x00], [0x8C, 0x17, 0x00],
    [0x5C, 0x2F, 0x00], [0x10, 0x45, 0x00], [0x05, 0x4A, 0x00], [0x00, 0x47, 0x2E],
    [0x00, 0x41, 0x66], [0x00, 0x00, 0x00], [0x05, 0x05, 0x05], [0x05, 0x05, 0x05],
    [0xC7, 0xC7, 0xC7], [0x00, 0x77, 0xFF], [0x21, 0x55, 0xFF], [0x82, 0x37, 0xFA],
    [0xEB, 0x2F, 0xB5], [0xFF, 0x29, 0x50], [0xFF, 0x22, 0x00], [0xD6, 0x32, 0x00],
    [0xC4, 0x62, 0x00], [0x35, 0x80, 0x00], [0x05, 0x8F, 0x00], [0x00, 0x8A, 0x55],
    [0x00, 0x99, 0xCC], [0x21, 0x21, 0x21], [0x09, 0x09, 0x09], [0x09, 0x09, 0x09],
    [0xFF, 0xFF, 0xFF], [0x0F, 0xD7, 0xFF], [0x69, 0xA2, 0xFF], [0xD4, 0x80, 0xFF],
    [0xFF, 0x45, 0xF3], [0xFF, 0x61, 0x8B], [0xFF, 0x88, 0x33], [0xFF, 0x9C, 0x12],
    [0xFA, 0xBC, 0x20], [0x9F, 0xE3, 0x0E], [0x2B, 0xF0, 0x35], [0x0C, 0xF0, 0xA4],
    [0x05, 0xFB, 0xFF], [0x5E, 0x5E, 0x5E], [0x0D, 0x0D, 0x0D], [0x0D, 0x0D, 0x0D],
    [0xFF, 0xFF, 0xFF], [0xA6, 0xFC, 0xFF], [0xB3, 0xEC, 0xFF], [0xDA, 0xAB, 0xEB],
    [0xFF, 0xA8, 0xF9], [0xFF, 0xAB, 0xB3], [0xFF, 0xD2, 0xB0], [0xFF, 0xEF, 0xA6],
    [0xFF, 0xF7, 0x9C], [0xD7, 0xE8, 0x95], [0xA6, 0xED, 0xAF], [0xA2, 0xF2, 0xDA],
    [0x99, 0xFF, 0xFC], [0xDD, 0xDD, 0xDD], [0x11, 0x11, 0x11], [0x11, 0x11, 0x11],
];

/// PPU
pub struct Ppu {
    /// フレームバッファ（RGBA形式、256x240）
    frame_buffer: Vec<u8>,
    /// 現在のスキャンライン
    scanline: u16,
    /// スキャンライン内のサイクル
    cycle: u16,
    /// フレームカウンタ
    frame: u64,
    /// アニメーション用カウンタ
    animation_frame: u32,
    
    // PPUレジスタ
    /// $2000: PPUCTRL
    ctrl: u8,
    /// $2001: PPUMASK
    mask: u8,
    /// $2002: PPUSTATUS
    status: u8,
    /// $2003: OAMADDR
    oam_addr: u8,
    /// $2005: PPUSCROLL (2回書き込み)
    scroll_x: u8,
    scroll_y: u8,
    scroll_latch: bool,
    /// $2006: PPUADDR (2回書き込み)
    addr_latch: bool,
    addr: u16,
    /// $2007用の内部データバッファ
    data_buffer: u8,
    
    /// VRAM (ネームテーブル 2KB + パレット 32バイト)
    vram: [u8; 0x1000],
    /// パレットRAM (32バイト)
    palette_ram: [u8; 0x20],
    
    /// 実際のレンダリングを行うかどうか（デモモード切り替え用）
    demo_mode: bool,
}

impl Ppu {
    /// 新しいPPUインスタンスを作成
    pub fn new() -> Self {
        Self {
            frame_buffer: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
            scanline: 0,
            cycle: 0,
            frame: 0,
            animation_frame: 0,
            ctrl: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            scroll_x: 0,
            scroll_y: 0,
            scroll_latch: false,
            addr_latch: false,
            addr: 0,
            data_buffer: 0,
            vram: [0; 0x1000],
            palette_ram: [0; 0x20],
            demo_mode: true,
        }
    }

    /// PPUをリセット
    pub fn reset(&mut self) {
        self.scanline = 0;
        self.cycle = 0;
        self.frame = 0;
        self.animation_frame = 0;
        self.ctrl = 0;
        self.mask = 0;
        self.status = 0;
        self.oam_addr = 0;
        self.scroll_x = 0;
        self.scroll_y = 0;
        self.scroll_latch = false;
        self.addr_latch = false;
        self.addr = 0;
        self.data_buffer = 0;
        self.demo_mode = false; // リセット時はデモモードをオフに
        
        // デフォルトパレットを初期化
        self.palette_ram[0] = 0x0F;  // 背景色（黒）
        self.palette_ram[1] = 0x00;  // 濃いグレー
        self.palette_ram[2] = 0x10;  // 明るいグレー  
        self.palette_ram[3] = 0x30;  // 白
        // 他のパレットも設定
        for i in 4..16 {
            self.palette_ram[i] = 0x0F;
        }
        
        // テスト用：ネームテーブルに簡単なパターンを設定
        for i in 0..960 {
            self.vram[i] = (i % 256) as u8;
        }
        
        // 画面をクリア
        for pixel in self.frame_buffer.chunks_mut(4) {
            pixel[0] = 0;
            pixel[1] = 0;
            pixel[2] = 0;
            pixel[3] = 255;
        }
        
        log::info!("PPU reset complete");
    }
    
    /// PPUレジスタ読み取り
    pub fn read_register(&mut self, addr: u16) -> u8 {
        match addr {
            0x2002 => {
                // PPUSTATUS
                let data = self.status;
                self.status &= 0x7F; // VBlankフラグをクリア
                self.addr_latch = false;
                self.scroll_latch = false;
                log::trace!("PPUSTATUS read: {:#04x}", data);
                data
            }
            0x2004 => {
                // OAMDATA
                0 // TODO: OAM実装
            }
            0x2007 => {
                // PPUDATA
                let addr = self.addr;
                
                // インクリメント
                let increment = if self.ctrl & 0x04 != 0 { 32 } else { 1 };
                self.addr = self.addr.wrapping_add(increment);
                
                // データ読み取り
                match addr {
                    0x0000..=0x1FFF => {
                        // CHR ROM/RAM - バッファリングされる
                        // 注：実際のCHR ROM読み取りはBusを通じて行う必要があるが、
                        // ここではbusにアクセスできないため、バッファの値のみ返す
                        let result = self.data_buffer;
                        // TODO: バスからCHR ROMを読み取ってバッファに格納
                        result
                    }
                    0x2000..=0x3EFF => {
                        // ネームテーブル - バッファリングされる
                        let result = self.data_buffer;
                        self.data_buffer = self.read_vram(addr);
                        result
                    }
                    0x3F00..=0x3FFF => {
                        // パレットRAM - 即座に読める（バッファしない）
                        // ただし、バッファにはミラーされたVRAMデータを入れる
                        self.data_buffer = self.read_vram(addr & 0x2FFF);
                        self.read_vram(addr)
                    }
                    _ => 0,
                }
            }
            _ => 0,
        }
    }
    
    /// PPUレジスタ書き込み
    pub fn write_register(&mut self, addr: u16, data: u8) {
        match addr {
            0x2000 => {
                // PPUCTRL
                self.ctrl = data;
                // ビット7が立ったらデモモードを終了
                if data != 0 {
                    self.demo_mode = false;
                }
                log::trace!("PPUCTRL write: {:#04x}", data);
            }
            0x2001 => {
                // PPUMASK
                self.mask = data;
                log::trace!("PPUMASK write: {:#04x}", data);
            }
            0x2003 => {
                // OAMADDR
                self.oam_addr = data;
            }
            0x2004 => {
                // OAMDATA
                // TODO: OAM実装
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            0x2005 => {
                // PPUSCROLL (2回書き込み)
                if !self.scroll_latch {
                    self.scroll_x = data;
                } else {
                    self.scroll_y = data;
                }
                self.scroll_latch = !self.scroll_latch;
            }
            0x2006 => {
                // PPUADDR (2回書き込み)
                if !self.addr_latch {
                    self.addr = ((data as u16) << 8) | (self.addr & 0xFF);
                } else {
                    self.addr = (self.addr & 0xFF00) | (data as u16);
                    // アドレスを$3FFFでマスク
                    self.addr &= 0x3FFF;
                }
                self.addr_latch = !self.addr_latch;
            }
            0x2007 => {
                // PPUDATA
                let addr = self.addr;
                
                // インクリメント
                let increment = if self.ctrl & 0x04 != 0 { 32 } else { 1 };
                self.addr = self.addr.wrapping_add(increment);
                
                // データ書き込み
                match addr {
                    0x0000..=0x1FFF => {
                        // CHR ROM/RAM
                        // 注：実際のCHR ROM書き込みはBusを通じて行う必要があるが、
                        // ここではbusにアクセスできないため、何もしない
                        // TODO: バスを通じてCHR ROM/RAMに書き込む
                    }
                    0x2000..=0x3EFF => {
                        // ネームテーブル
                        self.write_vram(addr, data);
                    }
                    0x3F00..=0x3FFF => {
                        // パレットRAM
                        self.write_vram(addr, data);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// 1PPUサイクル実行
    pub fn step(&mut self, bus: &mut Bus) {
        self.cycle += 1;
        if self.cycle > 340 {
            self.cycle = 0;
            self.scanline += 1;

            if self.scanline == 241 {
                // VBlankに入る
                self.status |= 0x80;
            } else if self.scanline > 261 {
                // フレーム終了、次のフレームへ
                self.scanline = 0;
                self.frame += 1;
                self.animation_frame = self.animation_frame.wrapping_add(1);
                
                // VBlankフラグをクリア
                self.status &= 0x7F;
                
                // 実際のレンダリングを実行
                if !self.demo_mode {
                    self.render(bus);
                } else {
                    self.render_demo_pattern();
                }
            }
        }
    }
    
    /// VRAM読み取り（PPU内部用）- ネームテーブルとパレット専用
    fn read_vram(&self, addr: u16) -> u8 {
        let addr = addr & 0x3FFF; // 14ビットアドレス空間
        match addr {
            0x2000..=0x3EFF => {
                // ネームテーブル
                let mirror_addr = self.mirror_vram_addr(addr);
                self.vram[mirror_addr]
            }
            0x3F00..=0x3FFF => {
                // パレットRAM - 直接読み取り（バッファを使わない）
                let palette_addr = (addr - 0x3F00) as usize & 0x1F;
                // 背景色のミラー: $10, $14, $18, $1C → $00
                let palette_addr = if palette_addr >= 0x10 && palette_addr % 4 == 0 {
                    palette_addr & 0x0F
                } else {
                    palette_addr
                };
                self.palette_ram[palette_addr]
            }
            _ => 0,
        }
    }
    
    /// VRAM書き込み（PPU内部用）- ネームテーブルとパレット専用
    fn write_vram(&mut self, addr: u16, data: u8) {
        let addr = addr & 0x3FFF;
        match addr {
            0x2000..=0x3EFF => {
                // ネームテーブル
                let mirror_addr = self.mirror_vram_addr(addr);
                self.vram[mirror_addr] = data;
            }
            0x3F00..=0x3FFF => {
                // パレットRAM
                let palette_addr = (addr - 0x3F00) as usize & 0x1F;
                // 背景色のミラー: $10, $14, $18, $1C → $00
                let palette_addr = if palette_addr >= 0x10 && palette_addr % 4 == 0 {
                    palette_addr & 0x0F
                } else {
                    palette_addr
                };
                self.palette_ram[palette_addr] = data;
            }
            _ => {}
        }
    }
    
    /// ネームテーブルアドレスのミラーリング処理（水平ミラーリングのみ対応）
    fn mirror_vram_addr(&self, addr: u16) -> usize {
        // $3000-$3EFFを$2000-$2EFFにミラー
        let mirrored_vram = addr & 0b10111111111111; // mirror down 0x3000-0x3eff to 0x2000 - 0x2eff
        let vram_index = mirrored_vram - 0x2000; // to vram vector
        let name_table = vram_index / 0x400; // to the name table index
        
        // 水平ミラーリング：
        // [ A ] [ a ]
        // [ B ] [ b ]
        match name_table {
            2 => (vram_index - 0x400) as usize,
            3 => (vram_index - 0x800) as usize,
            1 => (vram_index - 0x400) as usize,
            _ => vram_index as usize,
        }
    }
    
    /// 実際のレンダリング
    fn render(&mut self, bus: &mut Bus) {
        // デバッグ：強制的に背景をレンダリング（テスト用）
        log::debug!("Render called: PPUCTRL={:#04x}, PPUMASK={:#04x}", self.ctrl, self.mask);
        
        // 背景レンダリング（mask設定に関わらず一旦表示してみる）
        if self.mask & 0x08 != 0 || true { // 強制的に背景表示（テスト用）
            log::trace!("Rendering background, PPUCTRL={:#04x}, PPUMASK={:#04x}", self.ctrl, self.mask);
            self.render_background(bus);
        } else {
            // 背景が無効なら画面をクリア（背景色を使用）
            let bg_color = self.get_palette_color(0);
            log::trace!("Background disabled, clearing with color: {:?}", bg_color);
            for y in 0..SCREEN_HEIGHT {
                for x in 0..SCREEN_WIDTH {
                    let idx = (y * SCREEN_WIDTH + x) * 4;
                    self.frame_buffer[idx] = bg_color[0];
                    self.frame_buffer[idx + 1] = bg_color[1];
                    self.frame_buffer[idx + 2] = bg_color[2];
                    self.frame_buffer[idx + 3] = 255;
                }
            }
        }
        
        // スプライトレンダリング（将来実装）
        // if self.mask & 0x10 != 0 { ... }
    }
    
    /// 背景レンダリング
    fn render_background(&mut self, bus: &mut Bus) {
        // ネームテーブルのベースアドレス（PPUCTRL bit 0-1）
        let nametable_base = 0x2000 + ((self.ctrl & 0x03) as u16) * 0x0400;
        // パターンテーブルのベースアドレス（PPUCTRL bit 4）
        let pattern_table_base = if self.ctrl & 0x10 != 0 { 0x1000 } else { 0x0000 };
        
        log::debug!("Rendering background: nametable={:#06x}, pattern_table={:#06x}", 
                   nametable_base, pattern_table_base);
        
        // 30行 × 32列のタイル
        for row in 0..30 {
            for col in 0..32 {
                // ネームテーブルからタイル番号を取得
                let nametable_addr = nametable_base + row * 32 + col;
                let tile_num = self.read_vram(nametable_addr);
                
                // アトリビュートテーブルからパレット番号を取得
                let attr_addr = nametable_base + 0x3C0 + (row / 4) * 8 + (col / 4);
                let attr_byte = self.read_vram(attr_addr);
                
                // 2x2タイルブロック内の位置に応じてパレットを選択
                let shift = ((row % 4) / 2) * 4 + ((col % 4) / 2) * 2;
                let palette_num = (attr_byte >> shift) & 0x03;
                
                // パターンテーブルからタイルデータを取得してレンダリング
                self.render_tile(
                    bus,
                    tile_num,
                    col as usize * 8,
                    row as usize * 8,
                    pattern_table_base,
                    palette_num,
                );
            }
        }
    }
    
    /// 1タイル（8x8ピクセル）をレンダリング
    fn render_tile(
        &mut self,
        bus: &mut Bus,
        tile_num: u8,
        x: usize,
        y: usize,
        pattern_table_base: u16,
        palette_num: u8,
    ) {
        let tile_addr = pattern_table_base + (tile_num as u16) * 16;
        
        for row in 0..8 {
            // タイルデータは16バイト（8バイト×2プレーン）
            let plane0 = bus.ppu_read(tile_addr + row);
            let plane1 = bus.ppu_read(tile_addr + row + 8);
            
            for col in 0..8 {
                let bit = 7 - col;
                let pixel_value = ((plane0 >> bit) & 1) | (((plane1 >> bit) & 1) << 1);
                
                let px = x + col as usize;
                let py = y + row as usize;
                
                if px < SCREEN_WIDTH && py < SCREEN_HEIGHT {
                    // パレット番号とピクセル値から実際のパレットアドレスを計算
                    // pixel_value=0は透明色で、常に背景色($3F00)を使用
                    let palette_index = if pixel_value == 0 {
                        0
                    } else {
                        (palette_num * 4 + pixel_value) as usize
                    };
                    let color = self.get_palette_color(palette_index as u8);
                    
                    let idx = (py * SCREEN_WIDTH + px) * 4;
                    self.frame_buffer[idx] = color[0];
                    self.frame_buffer[idx + 1] = color[1];
                    self.frame_buffer[idx + 2] = color[2];
                    self.frame_buffer[idx + 3] = 255;
                }
            }
        }
    }
    
    /// パレットから色を取得
    fn get_palette_color(&self, index: u8) -> [u8; 3] {
        let palette_index = self.palette_ram[index as usize & 0x1F] as usize & 0x3F;
        PALETTE[palette_index]
    }
    
    /// デモパターンのレンダリング（感動的な表示）
    fn render_demo_pattern(&mut self) {
        let time = self.animation_frame as f32 * 0.08;
        
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                let idx = (y * SCREEN_WIDTH + x) * 4;
                
                // より動的なカラフル波パターン
                let dx = x as f32 - SCREEN_WIDTH as f32 / 2.0;
                let dy = y as f32 - SCREEN_HEIGHT as f32 / 2.0;
                let distance = (dx * dx + dy * dy).sqrt();
                let angle = dy.atan2(dx);
                
                // 複数の波を組み合わせる
                let wave1 = (distance * 0.05 - time).sin();
                let wave2 = (angle * 5.0 + time * 2.0).sin();
                let wave3 = ((x as f32 * 0.02 + time).sin() + (y as f32 * 0.02 + time).cos()) * 0.5;
                
                let combined = (wave1 + wave2 + wave3) / 3.0 * 0.5 + 0.5;
                let color_idx = ((combined * 63.0) as usize).min(63);
                
                let color = PALETTE[color_idx];
                self.frame_buffer[idx] = color[0];     // R
                self.frame_buffer[idx + 1] = color[1]; // G
                self.frame_buffer[idx + 2] = color[2]; // B
                self.frame_buffer[idx + 3] = 255;      // A
            }
        }
        
        // "HackNES.rs" のテキスト表示
        let pulse = ((time * 3.0).sin() * 0.3 + 0.7) as u8;
        self.draw_text_with_color("HackNES.rs", 80, 100, [255, pulse * 200 / 255, pulse * 150 / 255]);
        self.draw_text_with_color("NES Emulator", 70, 120, [pulse * 200 / 255, 255, pulse * 200 / 255]);
        
        // フレームカウンタを表示
        let frame_text = format!("Frame: {}", self.frame);
        self.draw_text_with_color(&frame_text, 10, 10, [255, 255, 255]);
        
        self.draw_text_with_color("is working!", 75, 140, [pulse * 150 / 255, pulse * 200 / 255, 255]);
    }
    
    /// カラー付きテキスト描画
    fn draw_text_with_color(&mut self, text: &str, start_x: usize, start_y: usize, color: [u8; 3]) {
        for (i, ch) in text.chars().enumerate() {
            self.draw_char_with_color(ch, start_x + i * 8, start_y, color);
        }
    }
    
    /// 簡易文字描画（8x8ピクセル）
    fn draw_char_with_color(&mut self, ch: char, x: usize, y: usize, color: [u8; 3]) {
        // 超簡易的なフォント（一部の文字のみ）
        let pattern = match ch {
            'H' => [0b11111111, 0b10000001, 0b10000001, 0b11111111, 0b10000001, 0b10000001, 0b10000001, 0b10000001],
            'a' => [0b00000000, 0b00000000, 0b01111110, 0b00000010, 0b01111110, 0b10000010, 0b01111110, 0b00000000],
            'c' => [0b00000000, 0b00000000, 0b01111110, 0b10000000, 0b10000000, 0b10000000, 0b01111110, 0b00000000],
            'k' => [0b00000000, 0b10000000, 0b10000010, 0b10000100, 0b11111000, 0b10000100, 0b10000010, 0b00000000],
            'N' => [0b11111111, 0b10000001, 0b10000001, 0b10000001, 0b10000001, 0b10000001, 0b10000001, 0b11111111],
            'E' => [0b11111111, 0b10000000, 0b10000000, 0b11111111, 0b10000000, 0b10000000, 0b11111111, 0b00000000],
            'S' => [0b01111110, 0b10000000, 0b10000000, 0b01111110, 0b00000010, 0b00000010, 0b01111110, 0b00000000],
            'r' => [0b00000000, 0b00000000, 0b10111100, 0b11000000, 0b10000000, 0b10000000, 0b10000000, 0b00000000],
            's' => [0b00000000, 0b00000000, 0b01111110, 0b10000000, 0b01111110, 0b00000010, 0b11111100, 0b00000000],
            'm' => [0b00000000, 0b00000000, 0b11111110, 0b10010010, 0b10010010, 0b10010010, 0b10010010, 0b00000000],
            'u' => [0b00000000, 0b00000000, 0b10000010, 0b10000010, 0b10000010, 0b10000010, 0b01111110, 0b00000000],
            'l' => [0b11000000, 0b01000000, 0b01000000, 0b01000000, 0b01000000, 0b01000000, 0b01111110, 0b00000000],
            't' => [0b01000000, 0b01000000, 0b11111110, 0b01000000, 0b01000000, 0b01000000, 0b01111110, 0b00000000],
            'o' => [0b00000000, 0b00000000, 0b01111110, 0b10000010, 0b10000010, 0b10000010, 0b01111110, 0b00000000],
            'i' => [0b00000000, 0b01000000, 0b00000000, 0b01000000, 0b01000000, 0b01000000, 0b01000000, 0b00000000],
            'n' => [0b00000000, 0b00000000, 0b10111100, 0b11000010, 0b10000010, 0b10000010, 0b10000010, 0b00000000],
            'g' => [0b00000000, 0b01111110, 0b10000010, 0b10000010, 0b01111110, 0b00000010, 0b01111110, 0b00000000],
            'w' => [0b00000000, 0b00000000, 0b10000010, 0b10010010, 0b10010010, 0b10010010, 0b01101100, 0b00000000],
            '!' => [0b01000000, 0b01000000, 0b01000000, 0b01000000, 0b01000000, 0b00000000, 0b01000000, 0b00000000],
            '.' => [0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b01000000, 0b00000000],
            ':' => [0b00000000, 0b01000000, 0b00000000, 0b00000000, 0b00000000, 0b01000000, 0b00000000, 0b00000000],
            'F' => [0b11111111, 0b10000000, 0b10000000, 0b11111111, 0b10000000, 0b10000000, 0b10000000, 0b00000000],
            '0' => [0b01111110, 0b10000010, 0b10000010, 0b10000010, 0b10000010, 0b10000010, 0b01111110, 0b00000000],
            '1' => [0b00010000, 0b00110000, 0b00010000, 0b00010000, 0b00010000, 0b00010000, 0b00111000, 0b00000000],
            '2' => [0b01111110, 0b10000010, 0b00000010, 0b01111110, 0b10000000, 0b10000000, 0b11111110, 0b00000000],
            '3' => [0b01111110, 0b10000010, 0b00000010, 0b01111110, 0b00000010, 0b10000010, 0b01111110, 0b00000000],
            '4' => [0b10000010, 0b10000010, 0b10000010, 0b11111110, 0b00000010, 0b00000010, 0b00000010, 0b00000000],
            '5' => [0b11111110, 0b10000000, 0b10000000, 0b11111110, 0b00000010, 0b00000010, 0b11111110, 0b00000000],
            '6' => [0b01111110, 0b10000000, 0b10000000, 0b11111110, 0b10000010, 0b10000010, 0b01111110, 0b00000000],
            '7' => [0b11111110, 0b00000010, 0b00000010, 0b00000010, 0b00000010, 0b00000010, 0b00000010, 0b00000000],
            '8' => [0b01111110, 0b10000010, 0b10000010, 0b01111110, 0b10000010, 0b10000010, 0b01111110, 0b00000000],
            '9' => [0b01111110, 0b10000010, 0b10000010, 0b01111110, 0b00000010, 0b00000010, 0b01111110, 0b00000000],
            ' ' => [0b00000000; 8],
            _ => [0b01111110, 0b01000010, 0b01000010, 0b01000010, 0b01000010, 0b01000010, 0b01111110, 0b00000000],
        };
        
        for row in 0..8 {
            for col in 0..8 {
                if pattern[row] & (1 << (7 - col)) != 0 {
                    let px = x + col;
                    let py = y + row;
                    if px < SCREEN_WIDTH && py < SCREEN_HEIGHT {
                        let idx = (py * SCREEN_WIDTH + px) * 4;
                        self.frame_buffer[idx] = color[0];     // R
                        self.frame_buffer[idx + 1] = color[1]; // G
                        self.frame_buffer[idx + 2] = color[2]; // B
                        self.frame_buffer[idx + 3] = 255;      // A
                    }
                }
            }
        }
    }

    /// フレームバッファの取得
    pub fn frame_buffer(&self) -> &[u8] {
        &self.frame_buffer
    }

    /// 現在のスキャンライン
    pub fn scanline(&self) -> u16 {
        self.scanline
    }

    /// 現在のサイクル
    pub fn cycle(&self) -> u16 {
        self.cycle
    }

    /// フレームカウンタ
    pub fn frame(&self) -> u64 {
        self.frame
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::Bus;

    #[test]
    fn test_ppu_creation() {
        let ppu = Ppu::new();
        assert_eq!(ppu.scanline, 0);
        assert_eq!(ppu.cycle, 0);
    }

    #[test]
    fn test_ppu_step() {
        let mut ppu = Ppu::new();
        let mut bus = Bus::new();
        
        for _ in 0..341 {
            ppu.step(&mut bus);
        }
        
        assert_eq!(ppu.scanline, 1);
        assert_eq!(ppu.cycle, 0);
    }
}
