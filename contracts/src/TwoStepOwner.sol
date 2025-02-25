// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

// TODO: Inspired by op-enclave owner management.

contract OwnerManaged {
    address public owner;
    address public manager;

    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    event ManagerChanged(address indexed previousManager, address indexed newManager);
    event ManagerRenounced(address indexed previousManager);
    event OwnershipRenounced(address indexed previousOwner);

    /// @notice Initializes the owner and manager.
    /// @dev The owner and manager are the same.
    constructor() {
        owner = msg.sender;
        manager = msg.sender;
    }

    /// @notice Only the owner can call the function.
    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert("Only the owner can call this function");
        }

        _;
    }

    /// @notice Only the manager can call the function.
    modifier onlyManager() {
        if (msg.sender != manager) {
            revert("Only the manager can call this function");
        }

        _;
    }

    /// @notice Only the owner or the manager can call the function.    
    modifier onlyOwnerOrManager() {
        if (msg.sender != owner && msg.sender != manager) {
            revert("Only the owner or manager can call this function");
        }

        _;
    }

    /// @notice Changes the owner.
    /// @dev Only the owner can change the owner.
    function transferOwnership(address newOwner) external onlyOwner {
        if (newOwner == address(0)) {
            revert("New owner cannot be the zero address");
        }

        owner = newOwner;

        emit OwnershipTransferred(owner, newOwner);
    }

    /// @notice Changes the manager.
    /// @dev Only the owner or the manager can change the manager.
    function changeManager(address newManager) external onlyOwnerOrManager {
        if (newManager == address(0)) {
            revert("New manager cannot be the zero address");
        }

        manager = newManager;

        emit ManagerChanged(manager, newManager);
    }

    /// @notice Renounces the owner role.
    /// @dev Only the owner can renounce the owner role.
    function renounceOwnership() external onlyOwner {
        owner = address(0);

        emit OwnershipRenounced(owner);
    }

    /// @notice Renounces the manager role.
    /// @dev Only the owner or the manager can renounce the manager role.
    function renounceManager() external onlyOwnerOrManager {
        manager = address(0);

        emit ManagerRenounced(manager);
    }
}
