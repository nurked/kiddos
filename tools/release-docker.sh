#!/bin/sh
# release-docker.sh: Linux (x86_64, aarch64) and Windows (x86_64) binaries,
# built in a local Docker container. Output in dist/.
#
#   tools/release-docker.sh
#
# The first run builds the image (a few minutes) and compiles everything
# from scratch (longer); later runs reuse the cargo cache volume.
set -e
cd "$(dirname "$0")/.."
docker build -t kiddos-build tools/docker
docker volume create kiddos-cargo-registry > /dev/null
docker volume create kiddos-target > /dev/null
docker run --rm \
  -v "$PWD:/src" \
  -v kiddos-cargo-registry:/usr/local/cargo/registry \
  -v kiddos-target:/src/target \
  -e JOBS="${JOBS:-4}" \
  kiddos-build sh tools/docker/build-inside.sh
