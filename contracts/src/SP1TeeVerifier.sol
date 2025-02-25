// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

import {NitroValidator} from "nitro-validator/NitroValidator.sol";
import {ICertManager} from "nitro-validator/ICertManager.sol";
import {ISP1Verifier} from "sp1-contracts/src/ISP1Verifier.sol";
import {LibCborElement, CborElement, CborDecode} from "nitro-validator/CborDecode.sol";
import {LibBytes} from "nitro-validator/LibBytes.sol";
import {OwnerManaged} from "./TwoStepOwner.sol";

contract SP1TeeVerifier is NitroValidator, OwnerManaged {
    using CborDecode for bytes;
    using LibCborElement for CborElement;
    using LibBytes for bytes;

    /// @notice Whether the PCR0 is valid.
    mapping(bytes32 => bool) public validPCR0s;

    /// @notice Whether the address is a signer.
    mapping(address => bool) public isSigner;

    /// @notice The SP1 verifier gateway contract.
    ISP1Verifier public immutable gateway;

    /// @notice The maximum age of an attestation.
    uint256 constant MAX_AGE = 1 days;

    constructor(address _certManager, address _gateway) NitroValidator(ICertManager(_certManager)) {
        gateway = ISP1Verifier(_gateway);
    }

    /// @notice Sets a valid PCR0 corresponding to a program that runs an SP1 executor.
    ///
    /// @dev Only the owner can set a valid PCR0.
    function setValidPCR0(bytes memory pcr0) external onlyOwner {
        validPCR0s[keccak256(pcr0)] = true;
    }

    /// @notice Adds a signer to the list of signers, after validating an attestation.
    ///
    /// @dev Only the owner or the manager can add a signer.
    function addSigner(bytes memory attestationTbs, bytes memory signature) external onlyOwnerOrManager {
        NitroValidator.Ptrs memory ptrs = validateAttestation(attestationTbs, signature);

        bytes32 pcr0 = attestationTbs.keccak(ptrs.pcrs[0]);
        require(validPCR0s[pcr0], "invalid pcr0 in attestation");

        require(ptrs.timestamp + MAX_AGE > block.timestamp, "attestation too old");

        // The publicKey is encoded in the form specified in section 4.3.6 of ANSI X9.62, which is a
        // 0x04 byte followed by the x and y coordinates of the public key. We ignore the first byte.
        bytes32 publicKeyHash = attestationTbs.keccak(ptrs.publicKey.start() + 1, ptrs.publicKey.length() - 1);
        address enclaveAddress = address(uint160(uint256(publicKeyHash)));
        isSigner[enclaveAddress] = true;
    }

    /// @notice Verifies a proof with given public values and vkey.
    /// @param programVKey The verification key for the RISC-V program.
    /// @param publicValues The public values encoded as bytes.
    /// @param proofBytes The proof of the program execution the SP1 zkVM encoded as bytes.
    function verifyProof(
        bytes32 programVKey,
        bytes calldata publicValues,
        bytes calldata proofBytes
    ) external view {
        // todo: verify signature of public values from the proof bytes.

        // The TEE verification was successful, now we verify the proof.
        gateway.verifyProof(programVKey, publicValues, proofBytes);
    }
}
