#!/usr/bin/env bash
# Rasterizes the published SVG into an ignored 1600-pixel review image.
set -euo pipefail
cd "$(dirname "$0")"
command -v rsvg-convert >/dev/null || {
  echo "install librsvg (rsvg-convert) to render the overview" >&2
  exit 1
}
[ -s ../aether_graphic.svg ] || { echo "run npm run build first" >&2; exit 1; }
mkdir -p .scratch
rsvg-convert -w 1600 -o .scratch/aether-preview.png ../aether_graphic.svg
printf '%s\n' 'Rendered .scratch/aether-preview.png'
