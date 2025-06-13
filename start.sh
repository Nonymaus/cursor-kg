#!/bin/bash

echo "Building the server..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "Build failed. Exiting."
    exit 1
fi

echo "Starting the server..."
cargo run --release --bin kg-mcp-server 