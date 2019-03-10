PROJECT=otlbook

all: ${PROJECT}_bg.wasm

${PROJECT}_bg.wasm:
	wasm-pack build wasm --release --target no-modules

run-server:
	lighttpd -D -f lighttpd.conf

test:
	cargo test --all

pkg/chunk.js: pkg/${PROJECT}_bg.wasm
	rm -f pkg/chunk.js
	echo "let code='`base64 -w0 $<`';" >> pkg/chunk.js

clean:
	rm -rf pkg/
	cargo clean