#!/usr/bin/env bash
# Vitro 项目级 lint 检查脚本
# 封装 Rust clippy / fmt 检查，便于本地验证与 CI 复用。

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT/native"

echo "==> cargo clippy --all-targets -- -D warnings"
cargo clippy --all-targets -- -D warnings

echo "==> cargo fmt --check"
cargo fmt --check

echo "==> TODO/FIXME/HACK 统计"
echo "Rust:"
grep -R "TODO\|FIXME\|HACK" src --include="*.rs" | wc -l
echo "Dart:"
grep -R "TODO\|FIXME\|HACK" "$PROJECT_ROOT/CideFlutter/lib" --include="*.dart" | wc -l

echo "==> All lint checks passed."
