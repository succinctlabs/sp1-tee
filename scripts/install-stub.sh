#!/bin/bash
# Install the SP1 TEE signers stub server.
# Runs as ec2-user (invoked from CDK user-data via `sudo -u ec2-user -H`).

set -euo pipefail

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
# ---------------------------------------------------------------------------
sudo cp sp1-tee-signers-stub.template.service \
    /etc/systemd/system/sp1-tee-signers-stub.service

sudo systemctl enable --now sp1-tee-signers-stub.service

echo "sp1-tee-signers-stub installed."
