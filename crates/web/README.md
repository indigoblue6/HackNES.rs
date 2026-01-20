# Web版のビルドとテスト

## 必要なツール

```bash
# wasm-packのインストール
cargo install wasm-pack

# 簡易HTTPサーバー（テスト用）
cargo install basic-http-server
```

## ビルド

```bash
# crates/web ディレクトリで実行
wasm-pack build --target web

# または、リリースビルド
wasm-pack build --target web --release
```

## 実行

```bash
# crates/web ディレクトリで実行
basic-http-server .
```

ブラウザで http://localhost:4000 を開く

## デバッグ

ブラウザの開発者ツール（F12）でコンソールを開くと、
Rustのログ出力を確認できます。
