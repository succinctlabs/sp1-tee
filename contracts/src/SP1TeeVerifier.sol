// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

import {ISP1Verifier} from "sp1-contracts/src/ISP1Verifier.sol";
import {SimpleOwnable} from "./SimpleOwnable.sol";

contract SP1TeeVerifier is SimpleOwnable {
    /// @notice Whether the address is a signer.
    mapping(address => bool) public isSigner;

    /// @notice The SP1 verifier gateway contract.
    ISP1Verifier public immutable gateway;

    constructor(address _gateway) {
        gateway = ISP1Verifier(_gateway);
    }

    /// @notice Adds a signer to the list of signers, after validating an attestation.
    ///
    /// @dev Only the owner can add a signer.
    function setIsSigner(address signer, bool _isSigner) external onlyOwner {
        isSigner[signer] = _isSigner;
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
