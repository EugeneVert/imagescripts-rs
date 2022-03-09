#!/bin/env bash

targets=("ims-rs")

export CARGO_TARGET_DIR=/tmp/cargo
cargo b --release
mkdir -p ./bin

for i in ${targets[*]}; do
    cp "/tmp/cargo/release/${i}" "./bin/"
done
strip -s ./bin/*

