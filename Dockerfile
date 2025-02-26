# ---- Build Stage ----
FROM public.ecr.aws/amazonlinux/amazonlinux:2023 AS builder

# Install system dependencies required to build Rust projects
RUN yum update -y \
    && yum install -y gcc clang

# Install Rust via rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR app

COPY install-guest.sh ./

# Make sure your install-guest script is executable and run it
RUN sed -i 's/sudo //g' ./install-guest.sh
RUN chmod +x ./install-guest.sh
RUN ./install-guest.sh

# Copy the entire Rust workspace into /app
COPY . ./

# Sanity check that cmake is installed.
RUN cmake --version

RUN cargo build --release --bin sp1-tee-enclave

# ---- Runtime Stage ----
FROM public.ecr.aws/amazonlinux/amazonlinux:2023

# Copy the binary from the build stage
COPY --from=builder /app/target/release/sp1-tee-enclave /usr/local/bin/sp1-tee-enclave

# Set the entrypoint to the enclave binary.
ENTRYPOINT ["/usr/local/bin/sp1-tee-enclave", "--enc-key-arn", "none"]