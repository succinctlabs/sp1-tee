// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

import {ISP1Verifier} from "sp1-contracts/src/ISP1Verifier.sol";
import {SimpleOwnable} from "./SimpleOwnable.sol";
import {IterableMap, SignersMap} from "./SignersMap.sol";

contract SP1TeeVerifier is SimpleOwnable {
    using IterableMap for SignersMap;

    /// @notice The signers map.
    SignersMap signersMap;

    /// @notice The SP1 verifier gateway contract.
    ISP1Verifier public immutable gateway;

    constructor(address _gateway) {
        gateway = ISP1Verifier(_gateway);
    }

    /// @notice Adds a signer to the list of signers, after validating an attestation.
    ///
    /// @dev Only the owner can add a signer.
    function addSigner(address signer) external onlyOwner {
        signersMap.addSigner(signer);
    }

    /// @notice Removes a signer from the list of signers.
    ///
    /// @dev Only the owner can remove a signer.
    function removeSigner(address signer) external onlyOwner {
        signersMap.removeSigner(signer);
    }

    /// @notice Returns the list of signers.
    ///
    /// @dev Only the owner can get the list of signers.
    function getSigners() external view returns (address[] memory) {
        return signersMap.getSigners();
    }

    /// @notice Returns if an address is a signer.
    function isSigner(address signer) external view returns (bool) {
        return signersMap.isSigner(signer);
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
