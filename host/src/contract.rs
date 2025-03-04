mod bindings {
    alloy::sol! {
        #[sol(rpc)]
        contract _TEEVerifier {
            /// @notice Adds a signer to the list of signers, after validating an attestation.
            ///
            /// @dev Only the owner or the manager can add a signer.
            function addSigner(address signer) external;

            /// @notice Returns the list of signers.
            function getSigners() external view returns (address[] memory);

            /// @notice Returns true if the address is a signer.
            function isSigner(address signer) external view returns (bool);
        }
    }
    
    pub type TEEVerifier<P, N> = _TEEVerifier::_TEEVerifierInstance<(), P, N>;
}

pub use bindings::TEEVerifier;