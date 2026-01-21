//! # PPU (Picture Processing Unit)
//! Based on https://github.com/starrhorne/nes-rust

use crate::cartridge::Cartridge;
use std::cell::RefCell;
use std::rc::Rc;

pub const SCREEN_WIDTH: usize = 256;
pub const SCREEN_HEIGHT: usize = 240;

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

pub struct Ppu {
    pub registers: Registers,
    pub renderer: Renderer,
    pub nmi: bool,
    cartridge: Option<Rc<RefCell<Cartridge>>>,
}

pub struct Registers {
    pub ctrl: u8,      // $2000
    pub mask: u8,      // $2001
    pub status: u8,    // $2002
    pub oam_addr: u8,  // $2003
    pub scroll_x: u8,  // $2005 first write
    pub scroll_y: u8,  // $2005 second write
    pub addr: u16,     // $2006
    pub data_buffer: u8,
    pub v: u16,        // Current VRAM address (15 bits)
    pub t: u16,        // Temporary VRAM address (15 bits)
    pub x: u8,         // Fine X scroll (3 bits)
    pub w: bool,       // Write toggle (1 bit)
}

pub struct Renderer {
    pub scanline: u16,
    pub cycle: u16,
    pub frame: u64,
    pub frame_buffer: Vec<u8>,
    pub palette: [u8; 32],
    pub vram: [u8; 2048],
    pub oam: [u8; 256],
}

impl Ppu {
    pub fn new() -> Self {
        Ppu {
            registers: Registers::new(),
            renderer: Renderer::new(),
            nmi: false,
            cartridge: None,
        }
    }

    pub fn set_cartridge(&mut self, cartridge: Rc<RefCell<Cartridge>>) {
        self.cartridge = Some(cartridge);
    }

    pub fn tick(&mut self) {
        self.renderer.tick();

        let scanline = self.renderer.scanline;
        let cycle = self.renderer.cycle;
        let rendering_enabled = (self.registers.mask & 0x18) != 0;

        // VBlank logic
        if scanline == 241 && cycle == 1 {
            self.registers.status |= 0x80; // Set VBlank flag
            if self.registers.ctrl & 0x80 != 0 {
                self.nmi = true;
            }
        }

        // Pre-render scanline (261)
        if scanline == 261 && cycle == 1 {
            self.registers.status &= !0x80; // Clear VBlank flag
            self.registers.status &= !0x40; // Clear Sprite 0 Hit flag
            self.registers.status &= !0x20; // Clear Sprite Overflow flag
            self.nmi = false;
        }

        // Sprite 0 Hit detection - check once per scanline at a fixed cycle
        // This simplified approach checks at cycle 2 of each visible scanline
        if scanline < 240 && cycle == 2 && rendering_enabled {
            self.check_sprite_0_hit_scanline(scanline);
        }

        // Render full frame at end of visible scanlines
        if scanline == 240 && cycle == 1 {
            self.render_frame();
        }

        // Clock MMC3 IRQ counter at cycle 260 on visible scanlines
        // Only clock when rendering is enabled (BG or sprites enabled)
        if cycle == 260
            && scanline < 240
            && rendering_enabled
        {
            if let Some(ref c) = self.cartridge {
                c.borrow_mut().clock_irq();
            }
        }
    }

    /// Simplified sprite 0 hit detection - checks if sprite 0 overlaps with background on this scanline
    fn check_sprite_0_hit_scanline(&mut self, scanline: u16) {
        // If already set, skip
        if (self.registers.status & 0x40) != 0 {
            return;
        }

        // Both BG and sprites must be enabled
        if (self.registers.mask & 0x08) == 0 || (self.registers.mask & 0x10) == 0 {
            return;
        }

        // Get sprite 0 info
        let sprite_y = self.renderer.oam[0] as u16;
        let sprite_tile = self.renderer.oam[1];
        let sprite_attr = self.renderer.oam[2];
        let sprite_x = self.renderer.oam[3] as u16;

        // Sprite 0 hit cannot occur at x=255
        if sprite_x >= 255 {
            return;
        }

        // Sprite appears on scanline (sprite_y + 1)
        let sprite_top = sprite_y.wrapping_add(1);
        let sprite_height: u16 = if (self.registers.ctrl & 0x20) != 0 { 16 } else { 8 };

        // Check if this scanline intersects sprite 0
        if scanline < sprite_top || scanline >= sprite_top + sprite_height {
            return;
        }

        // Calculate which row of the sprite we're on
        let sprite_row = scanline - sprite_top;
        let flip_v = (sprite_attr & 0x80) != 0;
        let flip_h = (sprite_attr & 0x40) != 0;
        let row = if flip_v { sprite_height - 1 - sprite_row } else { sprite_row };

        // Get sprite pattern
        let sprite_pattern_base: u16 = if (self.registers.ctrl & 0x08) != 0 { 0x1000 } else { 0x0000 };
        let tile_addr = sprite_pattern_base + (sprite_tile as u16) * 16 + row;
        let sprite_plane0 = self.read_chr(tile_addr);
        let sprite_plane1 = self.read_chr(tile_addr + 8);

        // Get background pattern table
        let bg_pattern_base: u16 = if (self.registers.ctrl & 0x10) != 0 { 0x1000 } else { 0x0000 };

        // Check each pixel of sprite 0 on this scanline
        for col in 0..8u16 {
            let pixel_x = sprite_x + col;
            
            // Cannot hit at x >= 255
            if pixel_x >= 255 {
                continue;
            }

            // Left edge clipping
            if pixel_x < 8 {
                if (self.registers.mask & 0x02) == 0 || (self.registers.mask & 0x04) == 0 {
                    continue;
                }
            }

            // Check sprite pixel
            let sprite_bit = if flip_h { col } else { 7 - col } as u8;
            let sprite_pixel = ((sprite_plane0 >> sprite_bit) & 1) | (((sprite_plane1 >> sprite_bit) & 1) << 1);
            
            if sprite_pixel == 0 {
                continue;
            }

            // Check background pixel at this position
            // For SMB, scroll is 0 when waiting for sprite 0 hit, so we use pixel_x directly
            let bg_x = pixel_x as usize;
            let bg_y = scanline as usize;
            
            let tile_x = bg_x / 8;
            let tile_y = bg_y / 8;
            let fine_x = bg_x % 8;
            let fine_y = bg_y % 8;

            // Get nametable address (assuming nametable 0 for simplicity, which is correct for SMB status bar)
            let nt_addr = 0x2000 + (tile_y * 32 + tile_x) as u16;
            let tile_num = self.read_vram(nt_addr);

            // Get background tile pixel
            let bg_tile_addr = bg_pattern_base + (tile_num as u16) * 16 + fine_y as u16;
            let bg_plane0 = self.read_chr(bg_tile_addr);
            let bg_plane1 = self.read_chr(bg_tile_addr + 8);
            let bg_bit = 7 - fine_x as u8;
            let bg_pixel = ((bg_plane0 >> bg_bit) & 1) | (((bg_plane1 >> bg_bit) & 1) << 1);

            // Hit if both pixels are non-transparent
            if bg_pixel != 0 {
                self.registers.status |= 0x40;
                return;
            }
        }
    }

    fn render_frame(&mut self) {
        // Clear frame buffer with background color
        let bg_color = self.get_palette_color(0);
        for pixel in self.renderer.frame_buffer.chunks_mut(4) {
            pixel[0] = bg_color[0];
            pixel[1] = bg_color[1];
            pixel[2] = bg_color[2];
            pixel[3] = 255;
        }

        // Render background
        if self.registers.mask & 0x08 != 0 {
            self.render_background();
        }

        // Render sprites
        if self.registers.mask & 0x10 != 0 {
            self.render_sprites();
        }
    }

    fn render_background(&mut self) {
        let pattern_table_base = if self.registers.ctrl & 0x10 != 0 { 0x1000 } else { 0x0000 };
        
        // Get base nametable offset from PPUCTRL bits 0-1
        // This adds 256 pixels horizontally (bit 0) or 240 pixels vertically (bit 1)
        let base_nt_x = if (self.registers.ctrl & 0x01) != 0 { 256 } else { 0 };
        let base_nt_y = if (self.registers.ctrl & 0x02) != 0 { 240 } else { 0 };

        for y in 0..240 {
            for x in 0..256 {
                // Combine base nametable offset with scroll values
                let scroll_x = (x + self.registers.scroll_x as usize + base_nt_x) % 512;
                let scroll_y = (y + self.registers.scroll_y as usize + base_nt_y) % 480;

                let tile_x = scroll_x / 8;
                let tile_y = scroll_y / 8;
                let pixel_x = scroll_x % 8;
                let pixel_y = scroll_y % 8;

                // Determine which nametable to use based on scroll position
                // Nametable layout: 0 1
                //                   2 3
                let nt_x = tile_x / 32;  // 0 or 1
                let nt_y = tile_y / 30;  // 0 or 1
                let nt_offset = ((nt_x + nt_y * 2) * 0x0400) as u16;
                let nt_addr = 0x2000 + nt_offset + ((tile_y % 30) * 32 + (tile_x % 32)) as u16;

                let tile_num = self.read_vram(nt_addr);

                // Get attribute
                let attr_x = (tile_x % 32) / 4;
                let attr_y = (tile_y % 30) / 4;
                let attr_addr = 0x2000 + nt_offset + 0x3C0 + (attr_y * 8 + attr_x) as u16;
                let attr_byte = self.read_vram(attr_addr);
                
                let shift = (((tile_y % 4) / 2) * 4 + ((tile_x % 4) / 2) * 2) as u8;
                let palette_num = (attr_byte >> shift) & 0x03;

                // Get pixel from pattern table
                let tile_addr = pattern_table_base + (tile_num as u16) * 16 + pixel_y as u16;
                let plane0 = self.read_chr(tile_addr);
                let plane1 = self.read_chr(tile_addr + 8);

                let bit = 7 - pixel_x;
                let pixel_value = ((plane0 >> bit) & 1) | (((plane1 >> bit) & 1) << 1);

                if pixel_value != 0 {
                    let palette_index = (palette_num * 4 + pixel_value) as usize;
                    let color = self.get_palette_color(palette_index as u8);
                    let idx = (y * SCREEN_WIDTH + x) * 4;
                    self.renderer.frame_buffer[idx] = color[0];
                    self.renderer.frame_buffer[idx + 1] = color[1];
                    self.renderer.frame_buffer[idx + 2] = color[2];
                    self.renderer.frame_buffer[idx + 3] = 255;
                }
            }
        }
    }

    fn render_sprites(&mut self) {
        let sprite_size = if self.registers.ctrl & 0x20 != 0 { 16 } else { 8 };
        let pattern_table_base = if self.registers.ctrl & 0x08 != 0 { 0x1000 } else { 0x0000 };

        // Render sprites in reverse order (priority)
        for i in (0..64).rev() {
            let sprite_y = self.renderer.oam[i * 4] as usize;
            let tile_num = self.renderer.oam[i * 4 + 1];
            let attributes = self.renderer.oam[i * 4 + 2];
            let sprite_x = self.renderer.oam[i * 4 + 3] as usize;

            if sprite_y >= 0xEF {
                continue;
            }

            let palette_num = (attributes & 0x03) + 4;
            let flip_h = (attributes & 0x40) != 0;
            let flip_v = (attributes & 0x80) != 0;

            for py in 0..sprite_size {
                let y = sprite_y + py + 1;
                if y >= 240 {
                    continue;
                }

                let tile_y = if flip_v { sprite_size - 1 - py } else { py };
                let tile_addr = pattern_table_base + (tile_num as u16) * 16 + tile_y as u16;
                let plane0 = self.read_chr(tile_addr);
                let plane1 = self.read_chr(tile_addr + 8);

                for px in 0..8 {
                    let x = sprite_x + px;
                    if x >= 256 {
                        continue;
                    }

                    let bit = if flip_h { px } else { 7 - px };
                    let pixel_value = ((plane0 >> bit) & 1) | (((plane1 >> bit) & 1) << 1);

                    if pixel_value != 0 {
                        let palette_index = (palette_num * 4 + pixel_value) as usize;
                        let color = self.get_palette_color(palette_index as u8);
                        let idx = (y * SCREEN_WIDTH + x) * 4;
                        self.renderer.frame_buffer[idx] = color[0];
                        self.renderer.frame_buffer[idx + 1] = color[1];
                        self.renderer.frame_buffer[idx + 2] = color[2];
                        self.renderer.frame_buffer[idx + 3] = 255;
                    }
                }
            }
        }
    }

    fn read_chr(&self, addr: u16) -> u8 {
        if let Some(ref c) = self.cartridge {
            c.borrow().read_chr_byte(addr)
        } else {
            0
        }
    }

    fn write_chr(&mut self, addr: u16, value: u8) {
        if let Some(ref c) = self.cartridge {
            c.borrow_mut().write_chr_byte(addr, value);
        }
    }

    fn read_vram(&self, addr: u16) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => self.read_chr(addr),
            0x2000..=0x3EFF => {
                let mirror_addr = self.mirror_vram_addr(addr);
                self.renderer.vram[mirror_addr]
            }
            0x3F00..=0x3FFF => {
                let palette_addr = (addr - 0x3F00) as usize & 0x1F;
                let palette_addr = if palette_addr >= 0x10 && palette_addr % 4 == 0 {
                    palette_addr & 0x0F
                } else {
                    palette_addr
                };
                self.renderer.palette[palette_addr]
            }
            _ => 0,
        }
    }

    fn write_vram(&mut self, addr: u16, value: u8) {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => self.write_chr(addr, value),
            0x2000..=0x3EFF => {
                let mirror_addr = self.mirror_vram_addr(addr);
                self.renderer.vram[mirror_addr] = value;
            }
            0x3F00..=0x3FFF => {
                let palette_addr = (addr - 0x3F00) as usize & 0x1F;
                let palette_addr = if palette_addr >= 0x10 && palette_addr % 4 == 0 {
                    palette_addr & 0x0F
                } else {
                    palette_addr
                };
                self.renderer.palette[palette_addr] = value;
            }
            _ => {}
        }
    }

    fn mirror_vram_addr(&self, addr: u16) -> usize {
        use crate::cartridge::Mirroring;

        let mirrored_vram = addr & 0b10111111111111;
        let vram_index = mirrored_vram - 0x2000;
        let name_table = vram_index / 0x400;

        let mirroring = if let Some(ref c) = self.cartridge {
            c.borrow().mirroring()
        } else {
            Mirroring::Horizontal
        };

        match mirroring {
            Mirroring::Horizontal => {
                match name_table {
                    1 => (vram_index - 0x400) as usize,
                    2 => (vram_index - 0x400) as usize,
                    3 => (vram_index - 0x800) as usize,
                    _ => vram_index as usize,
                }
            }
            Mirroring::Vertical => {
                match name_table {
                    2 => (vram_index - 0x800) as usize,
                    3 => (vram_index - 0x800) as usize,
                    _ => vram_index as usize,
                }
            }
            Mirroring::SingleScreenLower => {
                // All nametables map to the first one
                (vram_index % 0x400) as usize
            }
            Mirroring::SingleScreenUpper => {
                // All nametables map to the second one
                (vram_index % 0x400 + 0x400) as usize
            }
        }
    }

    fn get_palette_color(&self, index: u8) -> [u8; 3] {
        let palette_index = self.renderer.palette[index as usize & 0x1F] as usize & 0x3F;
        PALETTE[palette_index]
    }

    pub fn read_register(&mut self, addr: u16) -> u8 {
        match addr & 0x2007 {
            0x2002 => {
                let data = self.registers.status;
                self.registers.status &= 0x7F;
                self.registers.w = false;
                data
            }
            0x2004 => {
                self.renderer.oam[self.registers.oam_addr as usize]
            }
            0x2007 => {
                let addr = self.registers.v;
                let increment = if self.registers.ctrl & 0x04 != 0 { 32 } else { 1 };
                self.registers.v = self.registers.v.wrapping_add(increment);

                match addr {
                    0x0000..=0x3EFF => {
                        let result = self.registers.data_buffer;
                        self.registers.data_buffer = self.read_vram(addr);
                        result
                    }
                    0x3F00..=0x3FFF => {
                        self.registers.data_buffer = self.read_vram(addr & 0x2FFF);
                        self.read_vram(addr)
                    }
                    _ => 0,
                }
            }
            _ => 0,
        }
    }

    pub fn write_register(&mut self, addr: u16, value: u8) {
        match addr & 0x2007 {
            0x2000 => {
                self.registers.ctrl = value;
                // t: ...GH.. ........ <- d: ......GH
                self.registers.t = (self.registers.t & 0xF3FF) | ((value as u16 & 0x03) << 10);
            }
            0x2001 => self.registers.mask = value,
            0x2003 => self.registers.oam_addr = value,
            0x2004 => {
                self.renderer.oam[self.registers.oam_addr as usize] = value;
                self.registers.oam_addr = self.registers.oam_addr.wrapping_add(1);
            }
            0x2005 => {
                if !self.registers.w {
                    // First write (X scroll)
                    // t: ....... ...ABCDE <- d: ABCDE...
                    // x:              FGH <- d: .....FGH
                    self.registers.t = (self.registers.t & 0xFFE0) | ((value as u16) >> 3);
                    self.registers.x = value & 0x07;
                    self.registers.scroll_x = value;
                } else {
                    // Second write (Y scroll)
                    // t: FGH..AB CDE..... <- d: ABCDEFGH
                    self.registers.t = (self.registers.t & 0x8FFF) | (((value as u16) & 0x07) << 12);
                    self.registers.t = (self.registers.t & 0xFC1F) | (((value as u16) & 0xF8) << 2);
                    self.registers.scroll_y = value;
                }
                self.registers.w = !self.registers.w;
            }
            0x2006 => {
                if !self.registers.w {
                    // First write (high byte)
                    // t: .CDEFGH ........ <- d: ..CDEFGH
                    // t: X...... ........ <- 0
                    self.registers.t = (self.registers.t & 0x80FF) | (((value as u16) & 0x3F) << 8);
                } else {
                    // Second write (low byte)
                    // t: ....... ABCDEFGH <- d: ABCDEFGH
                    // v: <...all bits...> <- t: <...all bits...>
                    self.registers.t = (self.registers.t & 0xFF00) | (value as u16);
                    self.registers.v = self.registers.t;
                }
                self.registers.w = !self.registers.w;
            }
            0x2007 => {
                let addr = self.registers.v;
                let increment = if self.registers.ctrl & 0x04 != 0 { 32 } else { 1 };
                self.registers.v = self.registers.v.wrapping_add(increment);
                self.write_vram(addr, value);
            }
            _ => {}
        }
    }

    pub fn write_oam_data(&mut self, value: u8) {
        self.renderer.oam[self.registers.oam_addr as usize] = value;
        self.registers.oam_addr = self.registers.oam_addr.wrapping_add(1);
    }

    pub fn frame_buffer(&self) -> &[u8] {
        &self.renderer.frame_buffer
    }

    pub fn scanline(&self) -> u16 {
        self.renderer.scanline
    }

    pub fn cycle(&self) -> u16 {
        self.renderer.cycle
    }

    pub fn frame(&self) -> u64 {
        self.renderer.frame
    }
}

impl Registers {
    fn new() -> Self {
        Registers {
            ctrl: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            scroll_x: 0,
            scroll_y: 0,
            addr: 0,
            data_buffer: 0,
            v: 0,
            t: 0,
            x: 0,
            w: false,
        }
    }
}

impl Renderer {
    fn new() -> Self {
        Renderer {
            scanline: 0,
            cycle: 0,
            frame: 0,
            frame_buffer: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
            palette: [0; 32],
            vram: [0; 2048],
            oam: [0; 256],
        }
    }

    fn tick(&mut self) {
        self.cycle += 1;
        if self.cycle > 340 {
            self.cycle = 0;
            self.scanline += 1;
            if self.scanline > 261 {
                self.scanline = 0;
                self.frame += 1;
            }
        }
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}
