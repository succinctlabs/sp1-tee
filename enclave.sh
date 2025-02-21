set -e

if [ $1 == "terminate" ]; then
    nitro-cli terminate-enclave --all
    exit 0
fi

# Build the enclave.
docker build -t sp1-tee .

# Create the EIF from the enclave.
nitro-cli build-enclave --docker-uri sp1-tee:latest --output-file sp1-tee.eif

# todo!(n): Correct memory size.
# Run the enclave.

if [ $1 == "-f" ]; then
    nitro-cli run-enclave --cpu-count 2 --memory 700 --eif-path sp1-tee.eif --enclave-cid 10 --debug-mode
    # todo!(n): Correct memory size.
    nitro-cli console --enclave-name sp1-tee
else
    nitro-cli run-enclave --cpu-count 2 --memory 700 --eif-path sp1-tee.eif --enclave-cid 10
fi
