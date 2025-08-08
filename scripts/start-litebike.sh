#!/bin/bash

# Set the interface to swlan0
export LITEBIKE_INTERFACE=swlan0

# Set the bind address to 0.0.0.0
export LITEBIKE_BIND_ADDR=0.0.0.0

# Set the bind port to 8888
export LITEBIKE_BIND_PORT=8888

# Set the log level to debug
export LITEBIKE_LOG=debug

# Start the proxy
./target/release/litebike
