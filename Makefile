PROJECT=otlbook

# TODO: Remove in favor of Justfile

all: ${PROJECT}_bg.wasm target/release/olt

.PHONY: target/release/olt

target/release/olt:
	cargo build --release

${PROJECT}_bg.wasm:
	wasm-pack build wasm --release --target no-modules

run-server:
	lighttpd -D -f lighttpd.conf

test:
	cargo test --all

regenerate-test-outputs:
	# ONLY RUN THIS WHEN CHANGING Outline TYPE STRUCTURE
	# Re-dumps the test inputs using RON.
	# If you run this while there are known bugs in the parser, you will
	# invalidate tests.
	for X in parser/test/*.otl; do cargo run echo --debug < $$X > parser/test/`basename $$X .otl`.ron; done

pkg/chunk.js: pkg/${PROJECT}_bg.wasm
	rm -f pkg/chunk.js
	echo "let code='`base64 -w0 $<`';" >> pkg/chunk.js

clean:
	rm -rf pkg/
	cargo clean
