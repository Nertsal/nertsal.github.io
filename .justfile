# --out-dir ../website/static/background
build:
  cd background && cargo geng build --release --platform web
  cp -r background/target website/static/background
  cd website && zola build

serve: build
  cd website && zola serve
