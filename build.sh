#!/usr/bin/env bash
# Build the Rust core to WebAssembly, install frontend deps, and start Vite.
set -euo pipefail
cd "$(dirname "$0")"

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "error: wasm-pack not found." >&2
  echo "install it with:  cargo install wasm-pack" >&2
  exit 1
fi

echo "==> Building Rust -> WebAssembly (release)"
wasm-pack build --target web --out-dir web/pkg --out-name pacman --release

echo "==> Installing frontend dependencies"
cd web
npm install

echo "==> Starting dev server at http://localhost:5173"
npm run dev
