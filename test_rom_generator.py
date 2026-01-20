#!/usr/bin/env python3
"""
簡単なNESテストROM生成スクリプト
背景にカラフルなパターンを表示する最小限のROMを生成します。
"""

import struct

def create_test_rom():
    # iNESヘッダー (16バイト)
    header = bytearray([
        0x4E, 0x45, 0x53, 0x1A,  # "NES^Z"
        0x01,  # PRG-ROM: 1 x 16KB
        0x01,  # CHR-ROM: 1 x 8KB
        0x00,  # Flags 6: Mapper 0, 水平ミラーリング
        0x00,  # Flags 7
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00  # 残りのヘッダー
    ])
    
    # PRG-ROM (16KB)
    prg_rom = bytearray([0] * 16384)
    
    # リセットベクター($FFFC-$FFFD)に開始アドレス($8000)を設定
    prg_rom[0x3FFC] = 0x00
    prg_rom[0x3FFD] = 0x80
    
    # 簡単な初期化コード ($8000から)
    code = [
        # PPUの準備待ち (VBlankを2回待つ)
        0xA2, 0x02,        # LDX #$02
        # :vblankwait1
        0xAD, 0x02, 0x20,  # LDA $2002
        0x10, 0xFB,        # BPL :vblankwait1
        0xCA,              # DEX
        0xD0, 0xF8,        # BNE :vblankwait1
        
        # パレット設定
        0xA9, 0x3F,        # LDA #$3F
        0x8D, 0x06, 0x20,  # STA $2006
        0xA9, 0x00,        # LDA #$00
        0x8D, 0x06, 0x20,  # STA $2006
        
        # 背景パレットを設定 (16色)
        0xA2, 0x00,        # LDX #$00
        # :paletteloop
        0xBD, 0x50, 0x80,  # LDA palette,X ($8050)
        0x8D, 0x07, 0x20,  # STA $2007
        0xE8,              # INX
        0xE0, 0x10,        # CPX #$10
        0xD0, 0xF6,        # BNE :paletteloop
        
        # ネームテーブルにパターンを書き込み
        0xA9, 0x20,        # LDA #$20
        0x8D, 0x06, 0x20,  # STA $2006
        0xA9, 0x00,        # LDA #$00
        0x8D, 0x06, 0x20,  # STA $2006
        
        0xA2, 0x00,        # LDX #$00
        0xA0, 0x00,        # LDY #$00
        # :nameloop
        0x8A,              # TXA
        0x8D, 0x07, 0x20,  # STA $2007
        0xE8,              # INX
        0xD0, 0xFA,        # BNE :nameloop
        0xC8,              # INY
        0xC0, 0x04,        # CPY #$04
        0xD0, 0xF5,        # BNE :nameloop
        
        # アトリビュートテーブルを設定
        0xA9, 0x23,        # LDA #$23
        0x8D, 0x06, 0x20,  # STA $2006
        0xA9, 0xC0,        # LDA #$C0
        0x8D, 0x06, 0x20,  # STA $2006
        
        0xA2, 0x00,        # LDX #$00
        # :attrloop
        0xA9, 0xE4,        # LDA #$E4 (各パレット番号)
        0x8D, 0x07, 0x20,  # STA $2007
        0xE8,              # INX
        0xE0, 0x40,        # CPX #$40
        0xD0, 0xF6,        # BNE :attrloop
        
        # PPUを有効化
        0xA9, 0x90,        # LDA #$90 (NMI有効, BGパターン=$1000)
        0x8D, 0x00, 0x20,  # STA $2000
        0xA9, 0x1E,        # LDA #$1E (BG表示, スプライト表示)
        0x8D, 0x01, 0x20,  # STA $2001
        
        # 無限ループ
        # :forever
        0x4C, 0x78, 0x80,  # JMP :forever ($8078)
    ]
    
    for i, byte in enumerate(code):
        prg_rom[i] = byte
    
    # パレットデータ ($8050)
    palette = [
        0x0F, 0x00, 0x10, 0x30,  # 背景パレット0
        0x0F, 0x06, 0x16, 0x26,  # 背景パレット1
        0x0F, 0x09, 0x19, 0x29,  # 背景パレット2
        0x0F, 0x01, 0x11, 0x21,  # 背景パレット3
    ]
    
    for i, color in enumerate(palette):
        prg_rom[0x50 + i] = color
    
    # CHR-ROM (8KB) - タイルパターンを生成
    chr_rom = bytearray([0] * 8192)
    
    # いくつかのパターンを作成
    for tile in range(256):
        tile_offset = tile * 16
        
        # チェッカーボードパターン
        if tile % 2 == 0:
            for row in range(8):
                pattern = 0xAA if row % 2 == 0 else 0x55
                chr_rom[tile_offset + row] = pattern
                chr_rom[tile_offset + row + 8] = pattern
        else:
            # グラデーションパターン
            for row in range(8):
                pattern = (tile + row * 16) & 0xFF
                chr_rom[tile_offset + row] = pattern
                chr_rom[tile_offset + row + 8] = ~pattern & 0xFF
    
    # ROMファイルを出力
    with open('test.nes', 'wb') as f:
        f.write(header)
        f.write(prg_rom)
        f.write(chr_rom)
    
    print("test.nes を生成しました!")
    print(f"サイズ: {len(header) + len(prg_rom) + len(chr_rom)} bytes")

if __name__ == '__main__':
    create_test_rom()
