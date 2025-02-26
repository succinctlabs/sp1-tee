set -e

COMMANDS=("build" "run" "terminate" "count")

# Join all valid commands into a single space-padded string
VALID_COMMANDS=" ${COMMANDS[*]} "

# Single if-check for both "is $1 empty?" AND "is $1 not in COMMANDS?"
if [[ -z "$1" || ! $VALID_COMMANDS =~ " $1 " ]]; then
    echo "Usage: $0 [build [-f follow]] | run | terminate | count ]"
    echo "  build: Build the enclave"
    echo "  run: Build and run the enclave"
    echo "  terminate: Terminate all enclaves"
    echo "  count: Count the number of enclaves running"
    exit 1
fi

if [ $1 == "count" ]; then
    nitro-cli describe-enclaves | jq '. | length'
    exit 0
fi

# If the first argument is "terminate", terminate all enclaves.
if [ $1 == "terminate" ]; then
    nitro-cli terminate-enclave --all
    exit 0
fi

# Always build the enclave from scratch.
if [ $2 == "-f" || $2 == "--debug" ]; then
    docker build --build-arg DEBUG_MODE=1 -t sp1-tee .
else
    docker build -t sp1-tee .
fi

# Create the EIF from the enclave.
nitro-cli build-enclave --docker-uri sp1-tee:latest --output-file sp1-tee.eif

# Setup default values if not set.
if [ -z "$ENCLAVE_CPU_COUNT" ]; then
    ENCLAVE_CPU_COUNT=2
fi

if [ -z "$ENCLAVE_MEMORY" ]; then
    ENCLAVE_MEMORY=700
fi

if [ -z "$ENCLAVE_CID" ]; then
    ENCLAVE_CID=10
fi

RUN_COMMAND="nitro-cli run-enclave --cpu-count $ENCLAVE_CPU_COUNT --memory $ENCLAVE_MEMORY --eif-path sp1-tee.eif --enclave-cid $ENCLAVE_CID"

# Run the enclave, and optionally follow the logs.
if [[ $1 == "run" ]]; then
    # Note, logs are only available in debug mode.
    if [[ $2 == "-f" || $2 == "--debug" ]]; then
        $RUN_COMMAND --debug-mode
    else
        $RUN_COMMAND
    fi

    # sleep for a bit to ensure the enclave is running.
    sleep 2

    if [[ $2 == "-f" ]]; then
        nitro-cli console --enclave-name sp1-tee
    fi
fi