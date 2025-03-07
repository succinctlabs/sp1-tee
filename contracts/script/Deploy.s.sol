// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {SP1TeeVerifier} from "../src/SP1TeeVerifier.sol";

import {SP1VerifierGateway} from "sp1-contracts/src/SP1VerifierGateway.sol";
import {SP1Verifier} from "sp1-contracts/src/v4.0.0-rc.3/SP1VerifierPlonk.sol";

contract DeployScript is Script {
    function run() public {
        vm.startBroadcast();

        SP1TeeVerifier sp1TeeVerifier = new SP1TeeVerifier(msg.sender);

        // If we're deploying to Anvil, lets deploy the verifier gateway and add the verifier to it.
        if (block.chainid == 31337) {
            SP1VerifierGateway verifierGateway = new SP1VerifierGateway(msg.sender);
            SP1Verifier verifier = new SP1Verifier();

            verifierGateway.addRoute(address(verifier));
            verifierGateway.addRoute(address(sp1TeeVerifier));

            console.log("Deployed verifier gateway at %s", address(verifierGateway));
        }

        vm.stopBroadcast();

        string memory deploymentOutfile =
            string.concat(vm.projectRoot(), "/deployments/", vm.toString(block.chainid), ".json");

        vm.writeFile(deploymentOutfile, "");
        vm.writeJson({json: vm.serializeAddress("", "SP1TeeVerifier", address(sp1TeeVerifier)), path: deploymentOutfile});
    }
}
