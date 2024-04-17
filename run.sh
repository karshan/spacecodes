#!/bin/bash
set -e
cargo build
./target/debug/sc-server &
echo $! > server.pid
./target/debug/sc-client 127.0.0.1:8080 &
./target/debug/sc-client 127.0.0.1:8080
kill $(cat server.pid)
rm server.pid