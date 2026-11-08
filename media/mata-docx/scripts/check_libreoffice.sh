#!/usr/bin/env bash
set -euo pipefail
input="$(realpath "${1:?DOCX path required}")"
dir="$(mktemp -d)"
trap 'rm -rf "$dir"' EXIT
libreoffice -env:UserInstallation="file://$dir/profile" --headless --convert-to pdf --outdir "$dir" "$input"
test -s "$dir/$(basename "${input%.docx}").pdf"
pdfinfo "$dir/$(basename "${input%.docx}").pdf" | grep -E '^Pages: *[1-9]'
pdftotext "$dir/$(basename "${input%.docx}").pdf" "$dir/extracted.txt"
test -s "$dir/extracted.txt"
echo "LibreOffice PDF export produced selectable text"
