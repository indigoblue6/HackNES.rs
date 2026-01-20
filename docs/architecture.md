# HackNES.rs アーキテクチャドキュメント

## 概要

HackNES.rsは学習用のNESエミュレータプロジェクトです。単にゲームを動かすだけでなく、**内部状態を可視化**することで、NESのハードウェア動作を理解できるように設計されています。

## 設計思想

### 主要な設計原則

1. **可読性重視**: 最適化よりも理解しやすいコードを優先
2. **明確な分離**: CPU、PPU、Bus、Cartridgeなどのコンポーネントを明確に分離
3. **拡張性**: 新しいマッパーや機能を追加しやすい構造
4. **デバッグ機能**: CPU/PPUの内部状態を簡単に取得できるAPI

## プロジェクト構造

```
HackNES.rs/
├── Cargo.toml                # Workspace定義
├── crates/
│   ├── core/                 # エミュレータ本体
│   │   ├── src/
│   │   │   ├── lib.rs       # メインAPI
│   │   │   ├── cpu.rs       # 6502 CPU実装
│   │   │   ├── ppu.rs       # PPU実装
│   │   │   ├── bus.rs       # メモリバス
│   │   │   ├── cartridge.rs # カートリッジ/Mapper
│   │   │   ├── controller.rs # コントローラー入力
│   │   │   └── error.rs     # エラー型定義
│   │   └── Cargo.toml
│   ├── cli/                  # デスクトップ版 (SDL2)
│   │   ├── src/
│   │   │   └── main.rs
│   │   └── Cargo.toml
│   └── web/                  # Web版 (WASM)
│       ├── src/
│       │   └── lib.rs
│       ├── index.html
│       └── Cargo.toml
└── docs/
    └── architecture.md       # このドキュメント
```

## コアコンポーネント

### 1. CPU (`crates/core/src/cpu.rs`)

NESのCPUは6502のカスタム版（2A03）で、BCD（Binary-Coded Decimal）モードが無効化されています。

**主要な機能**:
- レジスタ管理（PC, SP, A, X, Y, Status）
- 命令のデコードと実行
- フラグ操作（N, V, B, D, I, Z, C）

**実装状況**:
- [x] 基本構造とレジスタ
- [ ] 全命令の実装（56種類の公式命令）
- [ ] 非公式命令（オプション）

### 2. PPU (`crates/core/src/ppu.rs`)

NESの描画を担当するチップ。256x240ピクセルの画面を生成します。

**主要な機能**:
- スキャンライン処理（262本、0-239が可視範囲）
- スプライトレンダリング（最大64個、1ライン8個制限）
- 背景レンダリング
- パレット管理

**実装状況**:
- [x] 基本構造とタイミング
- [ ] 背景レンダリング
- [ ] スプライトレンダリング
- [ ] スクロール処理

### 3. Bus (`crates/core/src/bus.rs`)

CPUとPPUのメモリアクセスを仲介し、各コンポーネント間の通信を管理します。

**CPUメモリマップ**:
```
$0000-$07FF: 内部RAM (2KB)
$0800-$1FFF: RAMのミラー
$2000-$2007: PPUレジスタ
$2008-$3FFF: PPUレジスタのミラー
$4000-$4017: APU/I/Oレジスタ
$4020-$FFFF: カートリッジ空間（ROM/RAM/Mapper）
```

**PPUメモリマップ**:
```
$0000-$1FFF: パターンテーブル（CHR ROM/RAM）
$2000-$2FFF: ネームテーブル
$3000-$3EFF: ネームテーブルのミラー
$3F00-$3FFF: パレットRAM
```

### 4. Cartridge (`crates/core/src/cartridge.rs`)

iNES形式のROMファイルを読み込み、マッパー（メモリバンク切り替え機構）を管理します。

**対応マッパー**:
- [x] Mapper 0 (NROM): 最も単純なマッパー、バンク切り替えなし
- [ ] Mapper 1 (MMC1): 多くのゲームで使用
- [ ] Mapper 2 (UxROM)
- [ ] Mapper 3 (CNROM)
- [ ] Mapper 4 (MMC3)

### 5. Controller (`crates/core/src/controller.rs`)

NESの標準コントローラー入力を管理します。

**ボタン**:
- A, B, Select, Start
- 十字キー（Up, Down, Left, Right）

## フロントエンド

### CLI版 (`crates/cli`)

SDL2を使用したデスクトップアプリケーション。

**機能**:
- ROMファイルの読み込み
- リアルタイム実行（約60 FPS）
- キーボード入力
- ウィンドウスケーリング

**使い方**:
```bash
cargo run -p nes_cli -- path/to/rom.nes
```

### Web版 (`crates/web`)

WebAssemblyを使用したブラウザ版。

**機能**:
- ROMファイルのドラッグ&ドロップ
- CPU/PPU状態の可視化
- ステップ実行（デバッグ用）

**ビルド**:
```bash
cd crates/web
wasm-pack build --target web
basic-http-server .
```

## 実装の優先順位

### フェーズ1: 基本構造（完了）
- [x] プロジェクト構造
- [x] CPUの基本構造
- [x] PPUの基本構造
- [x] Busの実装
- [x] Cartridge（Mapper 0）
- [x] フロントエンド（CLI/Web）

### フェーズ2: CPU命令実装（次のステップ）
- [ ] アドレッシングモード
- [ ] 全公式命令（56種類）
- [ ] 割り込み処理（IRQ, NMI, Reset）

### フェーズ3: PPU実装
- [ ] 背景レンダリング
- [ ] スプライトレンダリング
- [ ] スクロール
- [ ] V-Blank/NMI

### フェーズ4: 追加機能
- [ ] APU（音声）
- [ ] 追加のマッパー
- [ ] セーブステート
- [ ] リプレイ機能

## テスト戦略

### ユニットテスト
各コンポーネントに対して基本的なテストを実装：
- CPU命令の動作確認
- メモリアクセスのミラーリング
- ROMの読み込み

### 統合テスト
公開されているテストROMを使用：
- `nestest.nes`: CPU命令の包括的テスト
- `instr_test-v5`: 個別命令テスト
- `ppu_vbl_nmi`: PPUタイミングテスト

## リソース

### NESハードウェア仕様
- [NESDev Wiki](https://www.nesdev.org/wiki/Nesdev_Wiki)
- [6502 Reference](http://www.obelisk.me.uk/6502/)
- [NES Architecture](https://www.copetti.org/writings/consoles/nes/)

### テストROM
- [NES Test ROMs](https://github.com/christopherpow/nes-test-roms)
- [blargg's test roms](https://slack.net/~ant/nes-tests/)

## 開発ガイドライン

### コーディングスタイル
- Rustの標準的な命名規則に従う
- 公開APIには必ずドキュメントコメントを記述
- 複雑なロジックには説明コメントを追加
- `unwrap()`の使用を避け、適切なエラーハンドリング

### ログレベル
- `error`: エミュレーションを継続できないエラー
- `warn`: 予期しない動作だが継続可能
- `info`: 重要なイベント（ROM読み込み、リセットなど）
- `debug`: デバッグ用の詳細情報
- `trace`: 非常に詳細なトレース情報

### コミットメッセージ
```
[component] 簡潔な説明

詳細な説明（必要に応じて）
```

例:
```
[cpu] LDA命令の実装

即値、ゼロページ、絶対アドレスの3つのアドレッシングモードを実装。
Zero/Negativeフラグの更新処理を追加。
```

## まとめ

HackNES.rsは学習を目的としたNESエミュレータです。このドキュメントは、プロジェクトの構造と設計思想を理解するためのガイドとして作成されました。コードを読む際、新機能を追加する際の参考にしてください。
