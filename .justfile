list:
  just --list

build:
  cd background && cargo geng build --release --platform web --out-dir target
  wasm-opt -Os -o background/target/background.wasm background/target/background.wasm
  cp -r background/target website/static/background
  cd website && zola build

serve: build
  cd website && zola serve
