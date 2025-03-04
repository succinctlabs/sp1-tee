// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {SP1TeeVerifier} from "../src/SP1TeeVerifier.sol";

contract CounterScript is Script {
    function run() public {
        vm.startBroadcast();

        SP1TeeVerifier sp1TeeVerifier = new SP1TeeVerifier(msg.sender);

        vm.stopBroadcast();

        string memory deploymentOutfile =
            string.concat(vm.projectRoot(), "/deployments/", vm.toString(block.chainid), ".json");

        vm.writeFile(deploymentOutfile, "");
        vm.writeJson({json: vm.serializeAddress("", "SP1TeeVerifier", address(sp1TeeVerifier)), path: deploymentOutfile});
    }
}
