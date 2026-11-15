#!/usr/bin/env bash
# lz-zed — Zed 扩展构建脚本（macOS / Linux）
# 产物: grammars/lz.wasm
# 前置条件: Node.js >= 18, tree-sitter CLI（npx 自动拉取）
# 用法: ./build.sh

set -euo pipefail
cd "$(dirname "$0")"

echo "==> [1/3] tree-sitter generate"
( cd grammar && npx --yes tree-sitter generate )

echo "==> [2/3] tree-sitter build --wasm -> grammars/lz.wasm"
mkdir -p grammars
( cd grammar && npx --yes tree-sitter build --wasm -o ../grammars/lz.wasm ) \
  || ( cd grammar && npx --yes tree-sitter build-wasm . -o ../grammars/lz.wasm )

test -f grammars/lz.wasm || { echo "ERROR: grammars/lz.wasm 未生成"; exit 1; }

echo "==> [3/3] node test/run-tests.js"
node test/run-tests.js

echo ""
echo "构建完成: grammars/lz.wasm"
echo "下一步: 在 Zed 中执行 'zed: install dev extension' 选择本目录"
