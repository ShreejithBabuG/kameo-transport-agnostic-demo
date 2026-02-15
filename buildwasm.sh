#!/bin/bash
set -e

echo "Building WebAssembly client..."
cd ping-wasm-client
wasm-pack build --target web
cd ..

echo "Copying Wasm files to static directory..."
mkdir -p ping-http-server/static
cp ping-wasm-client/pkg/ping_wasm_client_bg.wasm ping-http-server/static/
cp ping-wasm-client/pkg/ping_wasm_client.js ping-http-server/static/

echo "Build complete!"
echo ""
echo "To run the demo:"
echo "   cargo run -p ping-http-server"
echo ""
echo "Then visit:"
echo "   JavaScript: http://localhost:8080"
echo "   WebAssembly: http://localhost:8080/static/wasm.html"