# CPSL Web Site

This directory contains the GitHub Pages site and the browser-hosted CPSL demo.

The demo runs CPSL in a Web Worker. The worker loads an Emscripten-built WASM
bundle from `web/dist/assets/wasm/` and calls the Rust C ABI exposed by
`web/cpsl-web`.

The browser sandbox shape is declared in `web/public/cpsl-web.toml`. The web
build derives the compiled module feature set from that manifest, and the Rust
wrapper reads its HTTP allow-list at compile time. The default HTTP allow-list
includes `httpbin.org` for simple request tests.

Each browser session gets a fresh writable in-memory filesystem mounted at the
sandbox root. Files are available to shell commands and `fs.*` calls for the
life of that session, then discarded on reset or page reload. The demo does not
use IndexedDB persistence. Synthetic system paths such as `/dev`, `/proc`, and
`/etc` remain read-only.

`build.sh` stamps the page, worker, and WASM URLs with a hash of the generated
WASM bundle so browsers do not keep running an older embedded runtime after a
deploy.

## Build

```sh
./web/build.sh
```

Requirements:

- Rust target: `wasm32-unknown-emscripten`
- Emscripten `emcc` from SDK 5.0.7 or newer

If a repo-local `emsdk/emsdk_env.sh` exists, `build.sh` sources it automatically.
Otherwise install and activate Emscripten from the repo root:

```sh
git clone https://github.com/emscripten-core/emsdk.git
./emsdk/emsdk install 5.0.7
./emsdk/emsdk activate 5.0.7
```

For static layout checks without building the WASM bundle:

```sh
CPSL_SKIP_WASM=1 ./web/build.sh
```

## Serve Locally

```sh
./web/server.sh
```

By default this serves `web/dist` at <http://127.0.0.1:8000>. Override the host
or port with `HOST` and `PORT`:

```sh
PORT=9000 ./web/server.sh
```

## Deploy

The workflow in `.github/workflows/pages.yml` builds `web/dist` and deploys it
with GitHub Pages. In the repository settings, set Pages source to GitHub
Actions. Custom domains can be configured later in the Pages settings.
