# 🎮 HackNES.rs

**Rustで実装する学習用NESエミュレータ**

HackNES.rsは、単にゲームを動かすだけでなく、**NESの内部構造を可視化し理解する**ことを目的とした教育的なエミュレータプロジェクトです。

## ✨ 特徴

- 🔍 **内部状態の可視化**: CPU/PPUのレジスタやメモリの状態をリアルタイムで確認
- 🏗️ **明確な設計**: CPU、PPU、Bus、Cartridgeを分離した読みやすいコード構造
- 🖥️ **2つのフロントエンド**: デスクトップ（SDL2）とブラウザ（WebAssembly）
- 📚 **学習重視**: 最適化よりも理解しやすさを優先した実装
- 🧪 **デバッグ機能**: ステップ実行やメモリダンプなど開発者向け機能

## 🚀 クイックスタート

### 必要な環境

- Rust 1.70以上
- SDL2（CLI版を使用する場合）

### ビルド

```bash
# プロジェクト全体のビルド
cargo build

# CLI版のビルド（最適化あり）
cargo build --release -p nes_cli

# Web版のビルド（wasm-packが必要）
cd crates/web
wasm-pack build --target web --release
```

### 実行

#### CLI版（デスクトップ）

```bash
# ROMファイルを指定して実行
cargo run -p nes_cli -- path/to/your/game.nes

# スケールファクタを指定（デフォルト: 3）
cargo run -p nes_cli -- path/to/your/game.nes --scale 4

# デバッグモードで実行
cargo run -p nes_cli -- path/to/your/game.nes --debug
```

#### Web版（ブラウザ）

```bash
# crates/webディレクトリで実行
cd crates/web
wasm-pack build --target web

# 簡易HTTPサーバーを起動（basic-http-serverが必要）
basic-http-server .

# ブラウザで http://localhost:4000 を開く
```

## 📁 プロジェクト構造

```
HackNES.rs/
├── Cargo.toml              # Workspace定義
├── crates/
│   ├── core/              # エミュレータ本体
│   │   ├── src/
│   │   │   ├── lib.rs     # メインAPI
│   │   │   ├── cpu.rs     # 6502 CPU実装
│   │   │   ├── ppu.rs     # PPU実装
│   │   │   ├── bus.rs     # メモリバス
│   │   │   ├── cartridge.rs # カートリッジ/Mapper
│   │   │   ├── controller.rs # コントローラー
│   │   │   └── error.rs   # エラー型
│   │   └── Cargo.toml
│   ├── cli/               # デスクトップ版
│   │   ├── src/main.rs
│   │   └── Cargo.toml
│   └── web/               # Web版
│       ├── src/lib.rs
│       ├── index.html
│       └── Cargo.toml
└── docs/
    └── architecture.md    # 詳細設計ドキュメント
```

## 🛠️ 実装状況

### コアコンポーネント

- [x] プロジェクト構造とワークスペース設定
- [x] CPU基本構造（6502レジスタ、フラグ）
- [x] PPU基本構造（スキャンライン管理）
- [x] メモリバス（CPUメモリマップ）
- [x] カートリッジ（iNES形式、Mapper 0）
- [x] コントローラー入力
- [ ] CPU命令セット（56種類の公式命令）
- [ ] PPUレンダリング（背景、スプライト）
- [ ] APU（音声）
- [ ] 追加マッパー（1, 2, 3, 4）

### フロントエンド

- [x] CLI版（SDL2ベース）
- [x] Web版（WASM + Canvas）
- [x] CPU/PPU状態表示
- [ ] メモリビューア
- [ ] ステップ実行
- [ ] ブレークポイント

## 🎯 次のステップ

1. **CPU命令の実装**: 6502の全公式命令を実装
2. **PPUレンダリング**: 背景とスプライトの描画
3. **テストROM**: 公開されているテストROMでの検証
4. **マッパー拡張**: より多くのゲームに対応

## 📚 ドキュメント

- [アーキテクチャドキュメント](docs/architecture.md) - 設計思想と実装の詳細
- [Web版ビルドガイド](crates/web/README.md) - WebAssembly版の構築方法

## 🧪 テスト

```bash
# 全テストの実行
cargo test

# 特定のクレートのテスト
cargo test -p nes_core
```

## 📖 学習リソース

HackNES.rsの開発に役立つリソース:

- [NESDev Wiki](https://www.nesdev.org/wiki/Nesdev_Wiki) - NESハードウェアの包括的なドキュメント
- [6502 Reference](http://www.obelisk.me.uk/6502/) - 6502 CPUの命令セット
- [NES Architecture](https://www.copetti.org/writings/consoles/nes/) - NESアーキテクチャの詳細解説

## 🤝 コントリビューション

このプロジェクトは学習目的で作成されています。バグ報告、機能提案、プルリクエストを歓迎します！

## 📄 ライセンス

MIT License - 詳細は[LICENSE](LICENSE)を参照してください。

## 🙏 謝辞

- NESDevコミュニティによる包括的なドキュメント
- 既存のRust製NESエミュレータプロジェクト
- テストROMを提供してくださった開発者の方々

---

**注意**: このエミュレータは教育目的で作成されています。商用ROMの使用は著作権法に従ってください。
