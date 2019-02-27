PROJECT=otlbook

all: ${PROJECT}_bg.wasm

${PROJECT}_bg.wasm:
	wasm-pack build wasm --release --target no-modules

run-server:
	lighttpd -D -f lighttpd.conf

# Run with CORS disabled so you you can load wasm from file:/// URLs
test-browser:
	mkdir -p /tmp/otlbook-scratchdir
	chromium --disable-web-security --user-data-dir=/tmp/otlbook-scratchdir `pwd` &!

pkg/chunk.js: pkg/${PROJECT}_bg.wasm
	rm -f pkg/chunk.js
	echo "let code='`base64 -w0 $<`';" >> pkg/chunk.js

clean:
	rm -rf wasm/pkg/
