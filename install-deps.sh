# Inspired by https://github.com/aws/aws-nitro-enclaves-sdk-c/blob/main/containers/Dockerfile.al2

sudo yum install -y \
	cmake3 \
	gcc \
	git \
	tar \
	make \
	gcc-c++ \
	go \
	ninja-build \
	doxygen

curl https://sh.rustup.rs -sSf | sh -s -- -y

source $HOME/.cargo/env

mkdir tmp
pushd tmp

# Install aws-lc
git clone --depth 1 -b v1.12.0 https://github.com/awslabs/aws-lc.git aws-lc
cmake3 -DCMAKE_PREFIX_PATH=/usr -DCMAKE_INSTALL_PREFIX=/usr -GNinja -DBUILD_TESTING=0 -S aws-lc -B aws-lc/build .
go env -w GOPROXY=direct
cmake3 --build aws-lc/build --parallel $(nproc) --target install

# Install s2n-tls
git clone --depth 1 -b v1.3.46 https://github.com/aws/s2n-tls.git
cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -S s2n-tls -B s2n-tls/build
cmake3 --build s2n-tls/build --parallel $(nproc) --target install

# Install aws-c-common
git clone --depth 1 -b v0.8.0 https://github.com/awslabs/aws-c-common.git
cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -GNinja -S aws-c-common -B aws-c-common/build
cmake3 --build aws-c-common/build --parallel $(nproc) --target install

# Install aws-c-sdkutils
git clone --depth 1 -b v0.1.2 https://github.com/awslabs/aws-c-sdkutils.git 
cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -GNinja -S aws-c-sdkutils -B aws-c-sdkutils/build
cmake3 --build aws-c-sdkutils/build --parallel $(nproc) --target install

# Install aws-c-cal
git clone --depth 1 -b v0.5.18 https://github.com/awslabs/aws-c-cal.git
cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -GNinja -S aws-c-cal -B aws-c-cal/build
cmake3 --build aws-c-cal/build --parallel $(nproc) --target install

# Install aws-c-io
git clone --depth 1 -b v0.11.0 https://github.com/awslabs/aws-c-io.git
cmake3 -DUSE_VSOCK=1 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -GNinja -S aws-c-io -B aws-c-io/build
cmake3 --build aws-c-io/build --parallel $(nproc) --target install

# Install aws-c-compression
git clone --depth 1 -b v0.2.14 http://github.com/awslabs/aws-c-compression.git
cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -GNinja -S aws-c-compression -B aws-c-compression/build
cmake3 --build aws-c-compression/build --parallel $(nproc) --target install

# Install aws-c-http
git clone --depth 1 -b v0.7.6 https://github.com/awslabs/aws-c-http.git
cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -GNinja -S aws-c-http -B aws-c-http/build
cmake3 --build aws-c-http/build --parallel $(nproc) --target install

# Install aws-c-auth
git clone --depth 1 -b v0.6.15 https://github.com/awslabs/aws-c-auth.git
cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -GNinja -S aws-c-auth -B aws-c-auth/build
cmake3 --build aws-c-auth/build --parallel $(nproc) --target install

# Install json-c
git clone --depth 1 -b json-c-0.16-20220414 https://github.com/json-c/json-c.git
cmake3 -DCMAKE_PREFIX_PATH=/usr -DBUILD_TESTING=0 -DCMAKE_INSTALL_PREFIX=/usr -DBUILD_SHARED_LIBS=OFF -GNinja -S json-c -B json-c/build
cmake3 --build json-c/build --parallel $(nproc)  --target install

# Install aws-nitro-enclaves-nsm-api
git clone --depth 1 -b v0.4.0 https://github.com/aws/aws-nitro-enclaves-nsm-api.git
source $HOME/.cargo/env && cd aws-nitro-enclaves-nsm-api && cargo build --release --jobs $(nproc) -p nsm-lib
mv aws-nitro-enclaves-nsm-api/target/release/libnsm.so /usr/lib64
mv aws-nitro-enclaves-nsm-api/target/release/nsm.h /usr/include

echo "Done installing dependencies"

popd
rm -rf tmp

# RUN cmake3 -DCMAKE_PREFIX_PATH=/usr -DCMAKE_INSTALL_PREFIX=/usr -GNinja \
# 	-S aws-nitro-enclaves-sdk-c -B aws-nitro-enclaves-sdk-c/build
# RUN cmake3 --build aws-nitro-enclaves-sdk-c/build --parallel $(nproc) --target install
# RUN cmake3 --build aws-nitro-enclaves-sdk-c/build --parallel $(nproc) --target docs

# # kmstool-enclave
# RUN mkdir -p /rootfs
# WORKDIR /rootfs

# RUN BINS="\
#     /usr/lib64/libnsm.so \
#     /usr/bin/kmstool_enclave \
#     " && \
#     for bin in $BINS; do \
#         { echo "$bin"; ldd "$bin" | grep -Eo "/.*lib.*/[^ ]+"; } | \
#             while read path; do \
#                 mkdir -p ".$(dirname $path)"; \
#                 cp -fL "$path" ".$path"; \
#             done \
#     done

# RUN mkdir -p /rootfs/etc/pki/tls/certs/ \
#     && cp -f /etc/pki/tls/certs/* /rootfs/etc/pki/tls/certs/
# RUN find /rootfs

# FROM scratch as kmstool-enclave

# COPY --from=builder /rootfs /

# ARG REGION
# ARG ENDPOINT
# ENV REGION=${REGION}
# ENV ENDPOINT=${ENDPOINT}
# CMD ["/usr/bin/kmstool_enclave"]

# # kmstool-instance
# FROM $BASE_IMAGE as kmstool-instance

# # TODO: building packages statically instead of cleaning up unwanted packages from amazonlinux
# RUN rpm -e python python-libs python-urlgrabber python2-rpm pygpgme pyliblzma python-iniparse pyxattr python-pycurl amazon-linux-extras yum yum-metadata-parser yum-plugin-ovl yum-plugin-priorities
# COPY --from=builder /usr/lib64/libnsm.so /usr/lib64/libnsm.so
# COPY --from=builder /usr/bin/kmstool_instance /kmstool_instance
# CMD ["/kmstool_instance"]

# # kmstool-enclave-cli
# FROM $BASE_IMAGE as kmstool-enclave-cli

# # TODO: building packages statically instead of cleaning up unwanted packages from amazonlinux
# RUN rpm -e python python-libs python-urlgrabber python2-rpm pygpgme pyliblzma python-iniparse pyxattr python-pycurl amazon-linux-extras yum yum-metadata-parser yum-plugin-ovl yum-plugin-priorities
# COPY --from=builder /usr/lib64/libnsm.so /usr/lib64/libnsm.so
# COPY --from=builder /usr/bin/kmstool_enclave_cli /kmstool_enclave_cli

# # Test
# FROM builder as test
# WORKDIR /tmp/crt-builder
# RUN cmake3 --build aws-nitro-enclaves-sdk-c/build --parallel $(nproc) --target test