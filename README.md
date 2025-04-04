# SP1 TEE Integrity Proofs

## Motivation

In order to provide assurance that even in the event of the SP1 proving system being compromised user applications are not affected,
integrity proofs offer signatures over the execution outputs of a given program.

The `SP1` executor is ran inside a Nitro TEE. The TEE holds a signing key that never leaves the encalve. The TEE then signs off on the public values and the verifying key of the program and inputs.

## Using SP1 TEE 2FA

For SP1 SDK usage, attestation verification and more information see the [SP1 docs.](https://docs.succinct.xyz/docs/network/developers/tee)

## Running your own TEE

This repo is built around Nitro Enclaves, which means it runs on an AWS EC2 instance.

By default the enclave withholds 9GB and 12vCPU, this mean you'll need a host machine with about 30GB of ram and 16vCPU, the actual memory requirements are not stated by AWS, you'll also need to ensure that the machine is in the proper IAM group for S3 access.

After ensuring that your EC2 instance meets these requirments, you must also enable the `Nitro Enclaves` setting in the Instance `Advanced Settings`. SSH into your machine and run the following command:

`sudo dnf install make`

`make install`

This will:
- Install the Nitro-CLI and Allocate the Enclave.
- Install the `server` system service, by default listening on port 8080.
- Start the `server`

If you dont want to use the production constants, you can run the following commands:

`make install-debug`
`DISABLE_ALERTS=1 cargo run --bin sp1-tee-server`

### ENV

Note that the `tracing-subscriber` implentation used in the server relies on the `alert-subscriber` crate. You will need to either pass the `DISABLE_ALERTS` env var or configrure the needed secrets for the service.
