#!/bin/sh
# Build the source-only Technical Preview artifact from tracked files.
set -eu

version=${VERSION:-0.1.0}
prefix=ngx-compress-rs-$version
output_dir=${OUTPUT_DIR:-dist}
archive=$output_dir/$prefix-source.zip

git diff --quiet
git diff --cached --quiet
[ -z "$(git status --porcelain --untracked-files=normal)" ] \
    || { echo 'working tree must be clean'; exit 1; }
mkdir -p "$output_dir"
[ ! -e "$archive" ] || { echo "$archive already exists"; exit 1; }

git archive --format=zip -9 --prefix="$prefix/" -o "$archive" HEAD
unzip -t "$archive" >/dev/null
unzip -l "$archive" > "$archive.contents.txt"
sha256sum "$archive" > "$archive.sha256"
NGINX_VERSION=1.30.4 scripts/toolchain-manifest.sh "$output_dir/toolchain.txt"

echo "$archive"
