# Inspired by https://github.com/aws/aws-nitro-enclaves-sdk-c/blob/main/containers/Dockerfile.al2
# todo!(n): proper attribution

#
# Installs the deps needed to build the `aws-nitro-enclave-c-sdk` and the `sp1-tee-enclave` crate.
# This script is intended to be run inside the enclave docker container, NOT on the host machine.
# 
# Dependencies:
# 1. aws-lc
# 2. s2n-tls
# 3. aws-c-common
# 4. aws-c-sdkutils
# 5. aws-c-cal
# 6. aws-c-io
# 7. aws-c-compression
# 8. aws-c-http
# 9. aws-c-auth
# 10. json-c
# 11. aws-nitro-enclaves-nsm-api
#

# Note: We include sudo so we can install it outside the docker container for sanity.

set -e

# Install the make deps.
sudo yum install -y \
	cmake3 \
	gcc \
	git \
	tar \
	make \
	gcc-c++ \
	go \
	ninja-build \
	doxygen \
	openssl-devel

# Install the rust toolchain.
curl https://sh.rustup.rs -sSf | sh -s -- -y

# Source the cargo env.
source $HOME/.cargo/env

# Create a tmp dir and treat it as the working directory.
mkdir tmp
pushd tmp

# Install aws-lc
git clone --depth 1 -b v1.12.0 https://github.com/awslabs/aws-lc.git aws-lc
sudo cmake3 -DCMAKE_PREFIX_PATH=/usr -DCMAKE_INSTALL_PREFIX=/usr -GNinja -DBUILD_TESTING=0 -S aws-lc -B aws-lc/build .
go env -w GOPROXY=direct
sudo cmake3 --build aws-lc/build --parallel $(nproc) --target install

# Install s2n-tls
git clone --depth 1 -b v1.3.46 https://github.com/aws/s2n-tls.git
sudo cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -S s2n-tls -B s2n-tls/build
sudo cmake3 --build s2n-tls/build --parallel $(nproc) --target install

# Install aws-c-common
git clone --depth 1 -b v0.8.0 https://github.com/awslabs/aws-c-common.git
sudo cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -GNinja -S aws-c-common -B aws-c-common/build
sudo cmake3 --build aws-c-common/build --parallel $(nproc) --target install

# Install aws-c-sdkutils
git clone --depth 1 -b v0.1.2 https://github.com/awslabs/aws-c-sdkutils.git 
sudo cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -GNinja -S aws-c-sdkutils -B aws-c-sdkutils/build
sudo cmake3 --build aws-c-sdkutils/build --parallel $(nproc) --target install

# Install aws-c-cal
git clone --depth 1 -b v0.5.18 https://github.com/awslabs/aws-c-cal.git
sudo cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -GNinja -S aws-c-cal -B aws-c-cal/build
sudo cmake3 --build aws-c-cal/build --parallel $(nproc) --target install

# Install aws-c-io
git clone --depth 1 -b v0.11.0 https://github.com/awslabs/aws-c-io.git
sudo cmake3 -DUSE_VSOCK=1 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -GNinja -S aws-c-io -B aws-c-io/build
sudo cmake3 --build aws-c-io/build --parallel $(nproc) --target install

# Install aws-c-compression
git clone --depth 1 -b v0.2.14 http://github.com/awslabs/aws-c-compression.git
sudo cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -GNinja -S aws-c-compression -B aws-c-compression/build
sudo cmake3 --build aws-c-compression/build --parallel $(nproc) --target install

# Install aws-c-http
git clone --depth 1 -b v0.7.6 https://github.com/awslabs/aws-c-http.git
sudo cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -GNinja -S aws-c-http -B aws-c-http/build
sudo cmake3 --build aws-c-http/build --parallel $(nproc) --target install

# Install aws-c-auth
git clone --depth 1 -b v0.6.15 https://github.com/awslabs/aws-c-auth.git
sudo cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -GNinja -S aws-c-auth -B aws-c-auth/build
sudo cmake3 --build aws-c-auth/build --parallel $(nproc) --target install

# Install json-c
git clone --depth 1 -b json-c-0.16-20220414 https://github.com/json-c/json-c.git
sudo cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -DBUILD_SHARED_LIBS=OFF -GNinja -S json-c -B json-c/build
sudo cmake3 --build json-c/build --parallel $(nproc)  --target install

# Install aws-nitro-enclaves-nsm-api
git clone --depth 1 -b v0.4.0 https://github.com/aws/aws-nitro-enclaves-nsm-api.git
source $HOME/.cargo/env && pushd aws-nitro-enclaves-nsm-api && cargo build --release --jobs $(nproc) -p nsm-lib
popd

# Mv the nsm lib and header to the system paths
sudo mv aws-nitro-enclaves-nsm-api/target/release/libnsm.so /usr/lib64
sudo mv aws-nitro-enclaves-nsm-api/target/release/nsm.h /usr/include

echo "Done installing dependencies"

popd
sudo rm -rf tmp