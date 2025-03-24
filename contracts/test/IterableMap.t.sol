// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {SignersMap, IterableMap} from "../src/SignersMap.sol";

contract IterableMapTest {
    using IterableMap for SignersMap;

    SignersMap signersMap;

    function setSigner(address signer) public {
        signersMap.addSigner(signer);
    }

    function removeSigner(address signer) public {
        signersMap.removeSigner(signer);
    }

    function getSigners() public view returns (address[] memory) {
        return signersMap.getSigners();
    }

    function getSignersLength() public view returns (uint256) {
        return signersMap.getSignersLength();
    }

    function signersIndex(address signer) public view returns (uint256) {
        return signersMap.signerIndex[signer];
    }
}

contract SP1TeeTest is Test {
    IterableMapTest iterableMapTest;

    function setUp() public {
        iterableMapTest = new IterableMapTest();
    }

    function test_addOneSigner() public {
        iterableMapTest.setSigner(address(1));
        assertEq(iterableMapTest.getSignersLength(), 1);
    }

    function test_removeOneSigner() public {
        iterableMapTest.setSigner(address(1));
        iterableMapTest.removeSigner(address(1));
        assertEq(iterableMapTest.getSignersLength(), 0);
    }

    function testAddManyThenRemoveOne() public {
        for (uint256 i = 0; i < 10; i++) {
            iterableMapTest.setSigner(address(uint160(i)));
        }

        assertEq(iterableMapTest.getSignersLength(), 10);

        iterableMapTest.removeSigner(address(1));
        assertEq(iterableMapTest.getSignersLength(), 9);
        assertEq(iterableMapTest.getSigners()[1], address(9));
    }

    function testAddManyThenRemoveAll() public {
        for (uint256 i = 0; i < 10; i++) {
            iterableMapTest.setSigner(address(uint160(i)));
        }

        assertEq(iterableMapTest.getSignersLength(), 10);

        for (uint256 i = 0; i < 5; i++) {
            iterableMapTest.removeSigner(address(uint160(i)));
        }

        assertEq(iterableMapTest.getSignersLength(), 5);

        // push another signer
        iterableMapTest.setSigner(address(uint160(10)));
        assertEq(iterableMapTest.getSignersLength(), 6);

        uint256 signersLength = iterableMapTest.getSignersLength();
        address[] memory signers = iterableMapTest.getSigners();
        for (uint256 i = 0; i < signersLength; i++) {
            iterableMapTest.removeSigner(signers[i]);
        }

        assertEq(iterableMapTest.getSignersLength(), 0);
    }

    function testCannotAddSignerTwice() public {
        iterableMapTest.setSigner(address(1));
        vm.expectRevert("Signer already exists");
        iterableMapTest.setSigner(address(1));
    }

    function testExpectedIndexAfterRemovingSigner() public {
        iterableMapTest.setSigner(address(1));
        iterableMapTest.setSigner(address(2));
        iterableMapTest.setSigner(address(3));
        iterableMapTest.setSigner(address(4));

        // Remove the first and second signer.
        iterableMapTest.removeSigner(address(1));
        iterableMapTest.removeSigner(address(2));

        // We should expect that the index of the removed signers is now 0 (removed from the list).
        assertEq(iterableMapTest.signersIndex(address(1)), 0);
        assertEq(iterableMapTest.signersIndex(address(2)), 0);

        // We should expect that the index of the fourth signer is now 0.
        // While the index is 0, its actually included in the list, this is fine
        // because we dont use the index to check inclusion.
        assertEq(iterableMapTest.signersIndex(address(4)), 0);

        // We should expect that the index of the third signer is now 1.
        assertEq(iterableMapTest.signersIndex(address(3)), 1);
    }

    function testRemoveLastSignerCorrectIndex() public {
        iterableMapTest.setSigner(address(1));
        iterableMapTest.setSigner(address(2));
        iterableMapTest.setSigner(address(3));
        iterableMapTest.removeSigner(address(3));

        assertEq(iterableMapTest.signersIndex(address(3)), 0);
    }

    function testCannotRemoveNonExistentSigner() public {
        vm.expectRevert("Signer does not exist");
        iterableMapTest.removeSigner(address(1));
    }
}
