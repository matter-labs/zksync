pragma solidity ^0.4.24;

import {PlasmaTransactor} from "./PlasmaTransactor.sol";
import {PlasmaDepositor} from "./PlasmaDepositor.sol";
import {PlasmaExitor} from "./PlasmaExitor.sol";

contract PlasmaContract is PlasmaDepositor, PlasmaExitor, PlasmaTransactor {}