
build: build-web build-native

prepare-web:
	cargo install -f wasm-bindgen-cli
	rustup target add wasm32-unknown-unknown

build-web: prepare-web
	cargo build --release --target wasm32-unknown-unknown
	wasm-bindgen --out-name assimilation --out-dir site --target web target/wasm32-unknown-unknown/release/assimilation.wasm

build-native:
	cargo build --release
