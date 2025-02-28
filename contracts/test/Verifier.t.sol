// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {SP1TeeVerifier} from "../src/SP1TeeVerifier.sol";
import {SP1VerifierGateway} from "sp1-contracts/src/SP1VerifierGateway.sol";
import {ISP1VerifierWithHash} from "sp1-contracts/src/ISP1Verifier.sol";
import {SP1Verifier as SP1VerifierPlonk} from "sp1-contracts/src/v4.0.0-rc.3/SP1VerifierPlonk.sol";

contract SP1TeeVerifierTest is Test {
    SP1TeeVerifier sp1TeeVerifier;
    SP1VerifierGateway sp1VerifierGateway;
    SP1VerifierPlonk sp1VerifierPlonk;

    uint256 realSigner = uint256(keccak256("messageSigner"));

    /// A valid proof for 4.0.0-rc.3
    bytes constant VALID_PROOF =
        hex"1b34fe11083bc4ddaac380b3fc5556107ab12fb886335ba7dd952f490e78582f27b05d640fcc2248d7f30e86c404029702fa8ddd8a0719e272823eb332725dca5a526f5e01a39d24f8b0c03977d9856a767bc1daf4db2f04b90eb9bd51c97a3f5db28f3019cfe88d2f476de1329f20cab785f3392ebf225246047caddb7db53abd12792b1dfff82dd93ba2a369f7253d89c491c58aa4846a2ab7f6a702fcb7ee8c0c6dc5159d42b993cd5cd4cf868b2eb030967b56c5b8eaad623b35adaf689b534fdfd52f51b39489e76e099ef01060c55435839ebaa66501b34727a0cc33def3c2a66a1eb4db53e4892d83d7eece567f2d9c9fd68f2fa5a61c60e098afd9a7834eb33419d4c563ab0ead4dbab716837895a8aa0e08d8197ff00476efa56d193617d3382711b02b4e2a69c7c2e352e2d18a3778af3b6ec6a101293d295e40a59a9baa6522348491916517804f9b5dbd44bb3495ca12026e4f06dd0bc5af8dd55d434fcc2bd58630bd4eccfc4560d3cda54f6ce8ab371b9f79578eb086fe888c08d0e39c17bcaa464d93c5971da5e8805bbcb848a28e62bcda6cfaf1fbb023c97b661a38165d2331a4894fd0eae9c7013d634987f551a1aa0d4e55f5e41d8f86ba6848ea231ac01a1ecba5cb627239bd20b837309fb9658d66b871b48c150d5e5469b6ec23d75b4b1f463dadae0e85be2bdabd764d332ed40240c0ab74f3376d450445ff29098332a265e6c402357e67d7237b251ce73748582c68965b68a216f34056d80b9f5c3173a70dc9e060c3df1bf4385f70ea08a7aceb9049fa15f573548e067107a5ca90a6c4d574ef4d51dfab690236b78745c3d74983c348793d1b9badfa4509f05ae9aa58f7a66b1d572834e49834735ca4903ac0781edaa44f320f07d3ca1606ba0e4d65fde596a0e1a0b0494866fec6a49a4c44130d47f21df01f91592e007517c70b49a8c5257d9bcc1f9f94c4c3bbb33ddecc2d91e6b6a6375138046608ee9e55ab972cedbb3b317a996fbda58a48cfc95d28cadcb37b23b5a45b26ca301b352d473ee9db26b330c083f0c50ddab7187d11af4f447eb8c4074631219017b98aeb967822a6041048acd2095b3f3dde95f1914d5b7e9a6db37fee6b25c5284b3abef7d2c12a2e5b1e35dda7b0df88dba42ca557f606b200af6874dfd1ba1d0398522a699f1e9060068fb765e4e23497f2489e2a85e431e01b0973d8a93c";
    bytes constant VALID_PUBLIC_VALUES = hex"0a0000003700000059000000";
    bytes32 constant VKEY = 0x001a070fe7311e5f070f87db45bc58157414b4fef11ae3be1735d6c908ccafeb;

    function setUp() public {
        sp1VerifierGateway = new SP1VerifierGateway(address(this));
        sp1TeeVerifier = new SP1TeeVerifier(address(sp1VerifierGateway));
        sp1VerifierPlonk = new SP1VerifierPlonk();

        // Add message signer as a signer.
        address messageSigner = vm.addr(uint256(realSigner));
        sp1TeeVerifier.addSigner(messageSigner);

        // Add TEE verifier as a route.
        sp1VerifierGateway.addRoute(address(sp1VerifierPlonk));
        sp1VerifierGateway.addRoute(address(sp1TeeVerifier));
    }

    function testProofFixtureIsValid() public view {
        sp1VerifierGateway.verifyProof(VKEY, VALID_PUBLIC_VALUES, VALID_PROOF);
    }

    /// @dev Verifies a proof that has the public values and vkey signed by an authorized signer.
    function testVerifyTEEProof() public view {
        bytes memory signedProof = signedProofBytes(realSigner, VKEY, VALID_PUBLIC_VALUES, VALID_PROOF);

        sp1VerifierGateway.verifyProof(VKEY, VALID_PUBLIC_VALUES, signedProof);
    }

    function testOnlyValidSignerCanSignProof(uint256 randomSigner) public {
        // Assume its not the real signer, and a valid scalar group element.
        vm.assume(
            randomSigner != realSigner
                && randomSigner < 115792089237316195423570985008687907852837564279074904382605163141518161494337
                && randomSigner > 0
        );

        bytes memory signedProof = signedProofBytes(randomSigner, VKEY, VALID_PUBLIC_VALUES, VALID_PROOF);

        address signer = vm.addr(randomSigner);
        vm.expectRevert(abi.encodeWithSelector(SP1TeeVerifier.InvalidSignature.selector, signer));
        sp1VerifierGateway.verifyProof(VKEY, VALID_PUBLIC_VALUES, signedProof);
    }

    function testOnlyOwnerCanAddSigner(address notTheOwner) public {
        vm.assume(notTheOwner != address(this));

        vm.startPrank(notTheOwner);
        vm.expectRevert("Only the owner can call this function");
        sp1TeeVerifier.addSigner(notTheOwner);
    }

    function testOnlyOwnerCanRemoveSigner(address notTheOwner) public {
        vm.assume(notTheOwner != address(this));

        vm.startPrank(notTheOwner);
        vm.expectRevert("Only the owner can call this function");
        sp1TeeVerifier.removeSigner(notTheOwner);
    }

    function testOnlyOwnerCanRenounceOwnership(address notTheOwner) public {
        vm.assume(notTheOwner != address(this));

        vm.startPrank(notTheOwner);
        vm.expectRevert("Only the owner can call this function");
        sp1TeeVerifier.renounceOwnership();
    }

    function testOnlyOwnerCanTransferOwnership(address notTheOwner) public {
        vm.assume(notTheOwner != address(this));

        vm.startPrank(notTheOwner);
        vm.expectRevert("Only the owner can call this function");
        sp1TeeVerifier.transferOwnership(notTheOwner);
    }

    /// @dev Returns a signed proof of the form:
    ///      [ tee_selector || v || r || s || proof ]
    ///
    /// @param vkey The verification key of the RISC-V program.
    /// @param publicValues The public values encoded as bytes.
    /// @param proof The proof of the program execution the SP1 zkVM encoded as bytes.
    function signedProofBytes(uint256 signer, bytes32 vkey, bytes memory publicValues, bytes memory proof)
        public
        view
        returns (bytes memory)
    {
        bytes4 selector = bytes4(ISP1VerifierWithHash(address(sp1TeeVerifier)).VERIFIER_HASH());
        console.logBytes4(selector);

        // The message is of the form keccak256([ vkey || publicValues ])
        bytes32 message_hash = keccak256(abi.encodePacked(vkey, publicValues));
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(signer, message_hash);

        // The proof the TEE verifier expects is of the form:
        // [ tee_selector || v || r || s || proof ]
        return abi.encodePacked(selector, v, r, s, proof);
    }
}
