// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract SimpleOwnable {
    /// @notice The owner of the contract.
    address public owner;

    /// @notice Emitted when the owner is transferred.
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);

    /// @notice Emitted when the owner is renounced.
    event OwnershipRenounced(address indexed previousOwner);

    /// @notice Initializes the owner.
    ///
    /// @dev The owner is the msg.sender.
    constructor() {
        owner = msg.sender;
    }

    /// @notice Only the owner can call the function.
    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert("Only the owner can call this function");
        }

        _;
    }

    /// @notice Changes the owner.
    ///
    /// @dev Only the owner can change the owner.
    function transferOwnership(address newOwner) external onlyOwner {
        if (newOwner == address(0)) {
            revert("New owner cannot be the zero address");
        }

        owner = newOwner;

        emit OwnershipTransferred(owner, newOwner);
    }

    /// @notice Renounces the owner role.
    ///
    /// @dev Only the owner can renounce the owner role.
    function renounceOwnership() external onlyOwner {
        address previousOwner = owner;
        owner = address(0);

        emit OwnershipRenounced(previousOwner);
    }
}
