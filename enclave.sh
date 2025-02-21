set -e

COMMANDS=("build" "run" "terminate")

# Join all valid commands into a single space-padded string
VALID_COMMANDS=" ${COMMANDS[*]} "

# Single if-check for both "is $1 empty?" AND "is $1 not in COMMANDS?"
if [[ -z "$1" || ! $VALID_COMMANDS =~ " $1 " ]]; then
    echo "Usage: $0 [build [-f follow]] | run | terminate ]"
    exit 1
fi

# If the first argument is "terminate", terminate all enclaves.
if [ $1 == "terminate" ]; then
    nitro-cli terminate-enclave --all
    exit 0
fi

# Always build the enclave from scratch.
docker build -t sp1-tee .

# Create the EIF from the enclave.
nitro-cli build-enclave --docker-uri sp1-tee:latest --output-file sp1-tee.eif

# todo!(n): Correct memory size.
RUN_COMMAND="nitro-cli run-enclave --cpu-count 2 --memory 700 --eif-path sp1-tee.eif --enclave-cid 10"

# Run the enclave, and optionally follow the logs.
if [[ $1 == "run" && $2 == "-f" ]]; then
    # Note, logs are only available in debug mode.
    $RUN_COMMAND --debug-mode

    # sleep for a bit to ensure the enclave is running.
    sleep 2

    nitro-cli console --enclave-name sp1-tee
elif [[ $1 == "run" ]]; then
    $RUN_COMMAND
fi