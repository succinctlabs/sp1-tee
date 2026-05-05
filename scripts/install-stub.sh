#!/bin/bash
# Install the SP1 TEE signers stub server.
# Runs as ec2-user (invoked from CDK user-data via `sudo -u ec2-user -H`).
#
# Required env vars:
#   SP1_TEE_SNAPSHOTS_BUCKET - S3 bucket holding the durable signers snapshot.
#                              Substituted into the systemd unit template
#                              and consumed by sp1-tee-signers-stub at runtime.

set -euo pipefail

if [ -z "${SP1_TEE_SNAPSHOTS_BUCKET:-}" ]; then
    echo "install-stub.sh: SP1_TEE_SNAPSHOTS_BUCKET is required" >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# Build deps. Full set so aws-lc-sys / cmake / native-bindgen crates compile
# cleanly on aarch64 (clang and perl in particular are needed by aws-lc-sys
# even when gcc is the primary toolchain).
# ---------------------------------------------------------------------------
sudo dnf install -y \
    gcc gcc-c++ \
    cmake make \
    perl clang \
    pkgconf-pkg-config \
    openssl-devel

# ---------------------------------------------------------------------------
# Swap. The release build of axum + aws-sdk-s3 + tokio peaks well over 2 GB;
# the t4g.medium host has 4 GB RAM, which is workable but not generous. Add
# 4 GB of swap as cheap insurance against an OOM-kill during `cargo install`.
# Idempotent across reruns and instance reboots.
# ---------------------------------------------------------------------------
if [ ! -f /swapfile ]; then
    sudo fallocate -l 4G /swapfile
    sudo chmod 600 /swapfile
    sudo mkswap /swapfile
fi

if ! sudo swapon --show=NAME | grep -qx /swapfile; then
    sudo swapon /swapfile
fi

if ! grep -q "^/swapfile " /etc/fstab; then
    echo "/swapfile none swap sw 0 0" | sudo tee -a /etc/fstab > /dev/null
fi

# ---------------------------------------------------------------------------
# Rust toolchain (installs into $HOME/.cargo).
# ---------------------------------------------------------------------------
curl https://sh.rustup.rs -sSf | sh -s -- -y
source "$HOME/.cargo/env"

# ---------------------------------------------------------------------------
# Build the stub binary.
# `--no-default-features` drops the `server` feature (Nitro Enclave / vsock
# deps); `signers-stub` pulls just `attestations + axum + tokio`; `production`
# selects the production S3 bucket constant.
# ---------------------------------------------------------------------------
cargo install --path host \
    --bin sp1-tee-signers-stub \
    --locked \
    --no-default-features \
    --features signers-stub,production

# ---------------------------------------------------------------------------
# Install systemd unit.
#
# The template carries `Environment=SP1_TEE_SNAPSHOTS_BUCKET=__SP1_TEE_SNAPSHOTS_BUCKET__`;
# substitute the placeholder with the bucket name supplied by CDK user-data.
# Bucket names are limited to [a-z0-9.-] so escaping with `|` as the sed
# delimiter is sufficient (no embedded metacharacters in practice).
# ---------------------------------------------------------------------------
sudo sed "s|__SP1_TEE_SNAPSHOTS_BUCKET__|${SP1_TEE_SNAPSHOTS_BUCKET}|g" \
    sp1-tee-signers-stub.template.service \
    | sudo tee /etc/systemd/system/sp1-tee-signers-stub.service > /dev/null

sudo systemctl enable --now sp1-tee-signers-stub.service

echo "sp1-tee-signers-stub installed (snapshots bucket: ${SP1_TEE_SNAPSHOTS_BUCKET})."
