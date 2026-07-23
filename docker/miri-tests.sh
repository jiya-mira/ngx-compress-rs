#!/bin/sh
# Miri undefined-behaviour / pointer-provenance check for the unsafe FFI crates.
#
# ngx-compress-core is checked hermetically in the Rust workflow. The unsafe
# raw-pointer code (from_raw_parts / .add() in ngx-compress-ffi and
# ngx-compress-module) needs the nginx bindings to compile, so it is checked
# here where an nginx source tree is available. Miri interprets Rust and aborts
# on real foreign-function calls; the exercised unit tests only manipulate
# repr(C) structs from Rust-owned memory, so no C is actually called.
set -eu

NGINX_SRC="${NGINX_SRC:-/opt/nginx-1.30.4}"
SRC=/tmp/ngx-miri
TOOLCHAIN="${RUSTUP_TOOLCHAIN:-nightly-2026-07-01}"

# Tree Borrows is the newer, sound-but-fewer-false-positives aliasing model and
# suits FFI-shaped pointer round-trips better than the default Stacked Borrows.
export MIRIFLAGS="${MIRIFLAGS:--Zmiri-tree-borrows}"

# nginx-sys derives its bindings from an already-configured tree (it reads
# <source>/objs, which pristine source lacks). Configure a throwaway copy.
rm -rf "$SRC"
cp -a "$NGINX_SRC" "$SRC"
( cd "$SRC" && ./configure --with-compat --with-http_v2_module >/tmp/cfg-miri.log 2>&1 ) \
    || { tail -60 /tmp/cfg-miri.log; exit 1; }
export NGINX_SOURCE_DIR="$SRC"

rustup component add miri --toolchain "$TOOLCHAIN"

# `--lib` restricts Miri to the in-crate unit tests (guard, buffer, worker,
# filter/buffer); integration tests and codec round-trips are out of scope.
cargo "+$TOOLCHAIN" miri test \
    -p ngx-compress-ffi \
    -p ngx-compress-module \
    --lib

echo 'PASS [miri]: no undefined behaviour or aliasing violation in FFI pointer code'
