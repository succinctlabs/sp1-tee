// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {SP1TeeVerifier} from "../src/SP1TeeVerifier.sol";
import {NitroValidator} from "nitro-validator/NitroValidator.sol";
import {CertManager, ICertManager} from "nitro-validator/CertManager.sol";

contract CounterScript is Script {
    function run() public {
        address sp1VerifierGateway = vm.envAddress("SP1_VERIFIER_GATEWAY");

        vm.startBroadcast();
        
        CertManager certManager = new CertManager();

        SP1TeeVerifier sp1TeeVerifier = new SP1TeeVerifier(address(certManager), sp1VerifierGateway);

        vm.stopBroadcast();

        string memory deploymentOutfile =
                string.concat(vm.projectRoot(), "/deployments/", vm.toString(block.chainid), ".json");
        
        vm.writeFile(deploymentOutfile, "");
        vm.writeJson({json: vm.serializeAddress("", "CertManager", address(certManager)), path: deploymentOutfile});
        vm.writeJson({json: vm.serializeAddress("", "SP1TeeVerifier", address(sp1TeeVerifier)), path: deploymentOutfile});
    }
}
