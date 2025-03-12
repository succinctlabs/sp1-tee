// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {SP1TeeVerifier} from "../src/SP1TeeVerifier.sol";

import {SP1VerifierGateway} from "sp1-contracts/src/SP1VerifierGateway.sol";
import {SP1Verifier} from "sp1-contracts/src/v4.0.0-rc.3/SP1VerifierPlonk.sol";

contract DeployScript is Script {
    function run() public {
        vm.startBroadcast();
        
        // The address of the verifier gateway.
        address gateway;

        // If we're deploying to Anvil, lets deploy the verifier gateway and add the verifier to it.
        // We always deploy a plonk verifier for anvil.
        if (block.chainid == 31337) {
            // Deploy a new gateway for anvil.
            SP1VerifierGateway verifierGateway = new SP1VerifierGateway(msg.sender);
            // Deploy the plonk verifier.
            SP1Verifier verifier = new SP1Verifier();
            // Add the plonk verifier to the gateway.
            verifierGateway.addRoute(address(verifier));

            console.log("Anvil: Deployed verifier gateway at %s", address(verifierGateway));

            gateway = address(verifierGateway);
        } else {
            gateway = vm.envAddress("SP1_VERIFIER_GATEWAY");
        }

        SP1TeeVerifier sp1TeeVerifier = new SP1TeeVerifier(gateway, msg.sender);

        vm.stopBroadcast();

        string memory deploymentOutfile =
            string.concat(vm.projectRoot(), "/deployments/", vm.toString(block.chainid), ".json");

        vm.writeFile(deploymentOutfile, "");
        vm.writeJson({json: vm.serializeAddress("", "SP1TeeVerifier", address(sp1TeeVerifier)), path: deploymentOutfile});
    }
}
