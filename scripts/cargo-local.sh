#!/bin/sh
# 不修改全局 shell 配置；优先使用本仓库安装的 Rust。
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
if [ -x "$ROOT/.toolchain/cargo/bin/cargo" ]; then
  export CARGO_HOME="$ROOT/.toolchain/cargo"
  export RUSTUP_HOME="$ROOT/.toolchain/rustup"
  export PATH="$CARGO_HOME/bin:$PATH"
fi
cd "$ROOT"
exec cargo "$@"
