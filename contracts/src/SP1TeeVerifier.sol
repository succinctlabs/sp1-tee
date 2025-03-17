// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1VerifierGateway, ISP1Verifier} from "sp1-contracts/src/ISP1VerifierGateway.sol";
import {SimpleOwnable} from "./SimpleOwnable.sol";
import {IterableMap, SignersMap} from "./SignersMap.sol";

/// @title SP1 Tee Verifier
/// @author Succinct Labs
/// @notice This contract is a wrapper around any SP1 verifier that additionally verifies
///         a signature over the public values and program vkey.
contract SP1TeeVerifier is ISP1Verifier, SimpleOwnable {
    using IterableMap for SignersMap;

    /// @notice Thrown when the proof bytes appear to be invalid.
    error WrongVerifierSelector(bytes4 receivedSelector, bytes4 expectedSelector);

    /// @notice Thrown when the signature is invalid.
    error InvalidSignature(address signer);

    /// @notice Thrown when the recovery id is invalid.
    error InvalidRecoveryId(uint8 v);

    /// @notice The signers map.
    SignersMap signersMap;

    /// @notice The verifier to delegate to.
    ISP1VerifierGateway immutable gateway;

    /// @notice The version of the verifier.
    uint256 public constant VERSION = 1;

    /// @notice Initializes the verifier, as well as the owner.
    constructor(address _gateway, address _owner) SimpleOwnable(_owner) {
        gateway = ISP1VerifierGateway(_gateway);
    }

    /// @notice Adds a signer to the list of signers, after validating an attestation.
    ///
    /// @dev Only the owner can add a signer.
    function addSigner(address signer) external onlyOwner {
        if (signer == address(0)) {
            revert("Signer cannot be the zero address");
        }
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

    /// @notice Returns the "hash of this verifier" for use by the gateway.
    ///
    /// @dev Since this is not a "real verifier" this is merely a constant used for identification.
    function VERIFIER_HASH() public pure returns (bytes32) {
        return keccak256(abi.encodePacked("SP1TeeVerifier"));
    }

    /// @notice Verifies a proof with given public values and vkey.
    /// @param programVKey The verification key for the RISC-V program.
    /// @param publicValues The public values encoded as bytes.
    /// @param proofBytes The proof of the program execution the SP1 zkVM encoded as bytes.
    ///
    /// @dev This function will gladly accept high-s signatures, it is the responsibility of the
    ///      application to prevent replay attacks.
    ///
    /// @dev For more information about signature related attacks see:
    ///      https://scsfg.io/hackers/signature-attacks
    function verifyProof(bytes32 programVKey, bytes calldata publicValues, bytes calldata proofBytes) external view {
        bytes4 receivedSelector = bytes4(proofBytes[:4]);
        bytes4 expectedSelector = bytes4(VERIFIER_HASH());
        if (receivedSelector != expectedSelector) {
            revert WrongVerifierSelector(receivedSelector, expectedSelector);
        }

        // Extract the recovery id and the signature from the proof bytes.
        uint8 v = uint8(proofBytes[4]); // 1 byte: v
        bytes32 r = bytes32(proofBytes[5:37]); // 32 bytes: r
        bytes32 s = bytes32(proofBytes[37:69]); // 32 bytes: s
        
        // Extract the version from the proof bytes.
        uint8 version_len = uint8(proofBytes[69]); // 1 byte: version_len
        bytes memory version = proofBytes[70:70 + version_len]; // version_len bytes: version

        // compute the expected hash of the message
        bytes32 message_hash = keccak256(abi.encodePacked(version, programVKey, publicValues));

        // Validate the recovery id.
        if (v != 27 && v != 28) {
            revert InvalidRecoveryId(v);
        }

        // Recover the signer from the signature.
        address signer = ecrecover(message_hash, v, r, s);
        if (signer == address(0)) {
            // note: ecrecover can return address(0) if the signature is invalid.
            revert InvalidSignature(signer);
        }

        // Verify the signer is in the signers map.
        if (!signersMap.isSigner(signer)) {
            revert InvalidSignature(signer);
        }

        // The TEE verification was successful, callback into the gateway
        // with the proof bytes stripped of the signature.
        //
        // Note: Assumes the caller is an ISP1Verifier.
        gateway.verifyProof(programVKey, publicValues, proofBytes[70 + version_len:]);
    }
}
