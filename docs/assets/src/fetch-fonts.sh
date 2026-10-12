#!/usr/bin/env bash
# Downloads the OFL fonts that build.mjs outlines into the SVG (not committed).
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p fonts
base=https://raw.githubusercontent.com/google/fonts/bd8f81ddb5c74d5c8897b36ad88b440266245103/ofl
for file in firasans/FiraSans-Regular.ttf firasans/FiraSans-Medium.ttf \
  firasans/FiraSans-SemiBold.ttf firasans/FiraSans-Bold.ttf firasans/FiraSans-Black.ttf \
  firamono/FiraMono-Regular.ttf firamono/FiraMono-Medium.ttf "caveat/Caveat%5Bwght%5D.ttf"; do
  name=$(basename "$file" | sed 's/%5B/[/; s/%5D/]/')
  if [ ! -s "fonts/$name" ]; then
    curl -fsSL "$base/$file" -o "fonts/$name.download"
    mv "fonts/$name.download" "fonts/$name"
  fi
done
