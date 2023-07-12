build:
  cd background && cargo geng build --release --target wasm32-unknown-unknown --out-dir ../website/static/background
  cp -r background/target website/static/background
  cd website && zola build

serve: build
  cd website && zola serve
