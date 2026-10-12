# Development

For contribution policy, see [CONTRIBUTING.md](../CONTRIBUTING.md).
For user installation, source builds, and version pinning, see
[installation](install.md).

The repository pins its Rust toolchain in [rust-toolchain.toml](../rust-toolchain.toml).
Use the pinned toolchain for reproducible builds.
The [Makefile](../Makefile) provides these entry points:

```sh
CARGO_BUILD_JOBS=4 cargo build -p aether
make test       # workspace tests
make ci         # full local verification
```

Run Cargo jobs sequentially on machines with limited memory. Full CI also
checks client schema drift, client tests, mock end-to-end behavior, performance,
and terminal captures. See [CI](ci.md) for prerequisites and optional lanes;
the container lane is opt-in.

The [architecture guide](../ARCHITECTURE.md) describes source ownership.
The [documentation index](README.md) separates current guides from historical
plans and measurements. Contributions should preserve native harness behavior,
explicit orchestration, visible failures, and small, testable changes.

## Overview graphic

The README illustration is built from
[`assets/src/aether_graphic.src.svg`](assets/src/aether_graphic.src.svg).
With Node.js/npm, Bash and curl installed, run from the repository root:

```sh
cd docs/assets/src
npm ci
npm run fonts
npm run build
npm run render
```

The font download uses a pinned Google Fonts revision (Fira Sans, Fira Mono
and Caveat, under the OFL). The build outlines text into shared glyph paths
in `docs/assets/aether_graphic.svg`, so the published image needs no fonts.
Commit both the source and rebuilt SVG. Fonts and `node_modules` are ignored.
Rendering needs `rsvg-convert` from librsvg and writes the ignored
`docs/assets/src/.scratch/aether-preview.png` at 1600 pixels wide; inspect it
for clipped text, connectors and mascot accessories after each edit.
`assets/src/render.html` also previews the published SVG through a plain image
when opened in a browser. Keep its title and description accessible, use
neutral icons, and check feature labels against the current implementation.
