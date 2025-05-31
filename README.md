sorastats
=========

[![sorastats](https://img.shields.io/crates/v/sorastats.svg)](https://crates.io/crates/sorastats)
[![Actions Status](https://github.com/sile/sorastats/workflows/CI/badge.svg)](https://github.com/sile/sorastats/actions)
![License](https://img.shields.io/crates/l/sorastats)

[WebRTC SFU Sora] の[統計情報][統計 API]をターミナルで閲覧するためのビューアです。

![sorastats demo](sorastats.gif)

[WebRTC SFU Sora]: https://sora.shiguredo.jp/
[統計 API]: https://sora-doc.shiguredo.jp/API#dacb9c


インストール
------------

### ビルド済みバイナリ

Linux および MacOS 向けにはビルド済みバイナリが[リリースページ]で提供されています。

```console
// Linux でビルド済みバイナリを取得する例
$ curl -L https://github.com/sile/sorastats/releases/download/0.3.0/sorastats-0.3.0.x86_64-unknown-linux-musl -o sorastats
$ chmod +x sorastats
$ ./sorastats
```

[リリースページ]: https://github.com/sile/sorastats/releases

### [Cargo] でインストール

[Rust] のパッケージマネージャである [Cargo] がインストール済みの場合には、以下のコマンドが利用可能です:

```console
$ cargo install sorastats
$ sorastats
```

[Rust]: https://www.rust-lang.org/
[Cargo]: https://doc.rust-lang.org/cargo/

使い方
------

第一引数に Sora の API の URL を指定してコマンドを実行してください:

```console
$ sorastats ${SORA_API_URL}
```

### 用語

- **Value**
  - 個々のコネクションの統計値
- **Sum**
  - 全てのコネクションの統計値の合算
  - 統計値の種類が数値以外の場合には存在しない
- **Delta/s**
  - 今回と前回の統計値（**Sum** or **Value**）の差分を毎秒単位に変換した値
  - 統計値の種類が数値以外の場合には存在しない

### 各種オプション

`$ sorastats --help` コマンドを叩くと、以下のようなヘルプメッセージが表示されます。

```console
$ sorastats
WebRTC SFU Sora の統計情報ビューア

Usage: sorastats [OPTIONS] <SORA_API_URL>

Example:
  $ sorastats http://localhost:5000/api

Arguments:
  <SORA_API_URL> 「Sora の API の URL（リアルタイムモード）」あるいは「過去に `--record` で記録したファイルのパス（リプレイモード）」

Options:
      --version                           Print version
  -h, --help                              Print help ('--help' for full help, '-h' for summary)
  -i, --polling-interval <INTEGER>        統計 API から情報を取得する間隔（秒単位） [default: 1]
  -p, --chart-time-period <INTEGER>       チャートの X 軸の表示期間（秒単位） [default: 60]
  -c, --connection-filter <REGEXP:REGEXP> 集計対象に含めるコネクションをフィルタするための正規表現 [default: .*:.*]
  -k, --stats-key-filter <REGEXP>         集計対象に含める統計項目をフィルタするための正規表現 [default: .*]
      --record <PATH>                     指定されたファイルに、取得した統計情報を記録する
      --global                            指定された場合は、クラスター全体の統計値を取得します
```
