// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Maths} from "./Maths.sol";

contract Mathematician {
    uint256 public number;

    function square() public view returns (uint256) {
        return Maths.square(number);
    }
}
