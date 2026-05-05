# CPSL Web Site

This directory contains the GitHub Pages site and the browser-hosted CPSL demo.

The demo runs CPSL in a Web Worker. The worker loads an Emscripten-built WASM
bundle from `web/dist/assets/wasm/` and calls the Rust C ABI exposed by
`web/cpsl-web`.

## Build

```sh
./web/build.sh
```

Requirements:

- Rust target: `wasm32-unknown-emscripten`
- Emscripten `emcc` from SDK 5.0.7 or newer

For static layout checks without building the WASM bundle:

```sh
CPSL_SKIP_WASM=1 ./web/build.sh
```

## Deploy

The workflow in `.github/workflows/pages.yml` builds `web/dist` and deploys it
with GitHub Pages. In the repository settings, set Pages source to GitHub
Actions. Custom domains can be configured later in the Pages settings.
