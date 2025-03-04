// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @dev A library for managing an iterable map of signers.
struct SignersMap {
    /// @dev Whether the address is a signer.
    mapping(address => bool) map;

    /// @dev The list of signers.
    address[] signers;

    /// @dev Keep track of the index of the signer in the `signers` array.
    /// @dev This way if we need to remove the signer, we can do so in constant time.
    mapping(address => uint256) signerIndex;
}

library IterableMap {
    function addSigner(SignersMap storage self, address signer) internal {
        if (self.map[signer]) {
            revert("Signer already exists");
        }

        // Toggle the signer status.
        self.map[signer] = true;

        // Add the signer to the list.
        self.signers.push(signer);

        // Set the index of the signer.
        self.signerIndex[signer] = self.signers.length - 1;
    }

    function removeSigner(SignersMap storage self, address signer) internal {
        if (!self.map[signer]) {
            revert("Signer does not exist");
        }

        // Toggle the signer status.
        self.map[signer] = false;

        // Delete the signer index.
        uint256 indexToRemove = self.signerIndex[signer];
        delete self.signerIndex[signer];

        if (self.signers.length == 1) {
            self.signers.pop();

            return;
        }

        // Remove the signer from the list, by replacing it with the last signer in the list.
        address lastSigner = self.signers[self.signers.length - 1];

        // Move the last signer to the index of the signer to remove.
        self.signers[indexToRemove] = lastSigner;

        // Update the index of the last signer.
        self.signerIndex[lastSigner] = self.signerIndex[signer];

        // Update the index of the signer.
        self.signers.pop();
    }

    function isSigner(SignersMap storage self, address signer) internal view returns (bool) {
        return self.map[signer];
    }

    function getSigners(SignersMap storage self) internal view returns (address[] memory) {
        return self.signers;
    }

    function getSignersLength(SignersMap storage self) internal view returns (uint256) {
        return self.signers.length;
    }
}
