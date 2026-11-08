#!/usr/bin/env sh
set -eu
cargo test --all-targets --offline
cargo run --example showcase --offline
cargo run --example stages123 --offline
python3 scripts/verify_package.py showcase.docx stages123.docx
if command -v libreoffice >/dev/null 2>&1; then
    bash scripts/check_libreoffice.sh showcase.docx
fi
