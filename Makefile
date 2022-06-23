
BUILD_PREP_MARKER=.build_prep_v1

build: build-web build-native

$(BUILD_PREP_MARKER):
	cargo install -f wasm-bindgen-cli
	rustup target add wasm32-unknown-unknown
	touch $(BUILD_PREP_MARKER)

build-web: $(BUILD_PREP_MARKER)
	cargo build --release --target wasm32-unknown-unknown
	wasm-bindgen --out-name assimilation --out-dir site --target web target/wasm32-unknown-unknown/release/assimilation.wasm

build-native:
	cargo build
