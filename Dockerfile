# ---- Build Stage ----
FROM public.ecr.aws/amazonlinux/amazonlinux:2023 AS builder

# Install system dependencies required to build Rust projects
RUN yum update -y \
    && yum install -y gcc clang

# Install Rust via rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

ENV PATH="/root/.cargo/bin:${PATH}"

# Set the working directory
WORKDIR /app

COPY install-deps.sh .

# Make sure your install-deps script is executable and run it
RUN sed -i 's/sudo //g' install-deps.sh
RUN chmod +x install-deps.sh
RUN ./install-deps.sh

# Copy the entire Rust workspace into /app
COPY . .

# Sanity check that cmake is installed.
RUN cmake --version

RUN cargo build --release --bin sp1-tee-enclave

# ---- Runtime Stage ----
FROM public.ecr.aws/amazonlinux/amazonlinux:2023

# Copy the binary from the build stage
COPY --from=builder /app/target/release/sp1-tee-enclave /usr/local/bin/sp1-tee-enclave

# Set the entrypoint to the enclave binary.
ENTRYPOINT ["/usr/local/bin/sp1-tee-enclave"]