pragma solidity >=0.5.0 <0.7.0;

import "./Governance.sol";
import "./Proxy.sol";
import "./UpgradeGatekeeper.sol";
import "./ZkSync.sol";
import "./Verifier.sol";
import "./TokenInit.sol";

contract DeployFactory is TokenDeployInit {

    // Why do we deploy contracts in the constructor?
    //
    // If we want to deploy Proxy and UpgradeGatekeeper (using new) we have to deploy their contract code with this contract
    // in total deployment of this contract would cost us around 2.5kk of gas and calling final transaction
    // deployProxyContracts would cost around 3.5kk of gas(which is equivalent but slightly cheaper then doing deploy old way by sending
    // transactions one by one) but doing this in one method gives us simplicity and atomicity of our deployment.
    //
    // If we use selfdesctruction in the constructor then it removes overhead of deploying Proxy and UpgradeGatekeeper
    // with DeployFactory and in total this constructor would cost us around 3.5kk, so we got simplicity and atomicity of
    // deploy without overhead.
    constructor(
        Governance _govTarget, Verifier _verifierTarget, ZkSync _zkSyncTarget,
        bytes32 _genesisRoot, address _firstValidator, address _governor
    ) public {
        deployProxyContracts(_govTarget, _verifierTarget, _zkSyncTarget, _genesisRoot, _firstValidator, _governor);

        selfdestruct(msg.sender);
    }

    event Addresses(address governance, address zksync, address verifier, address gatekeeper);


    function deployProxyContracts(Governance _gov, Verifier _ver, ZkSync _zks, bytes32 _genesisRoot, address _validator, address _governor) internal {

        Proxy gov = new Proxy(address(_gov), abi.encode(this));
        // set this contract as governor
        Proxy ver = new Proxy(address(_ver), abi.encode());
        Proxy zks = new Proxy(address(_zks), abi.encode(address(gov), address(ver), _genesisRoot));

        UpgradeGatekeeper gk = new UpgradeGatekeeper(zks);

        gov.transferMastership(address(gk));
        gk.addUpgradeable(address(gov));

        ver.transferMastership(address(gk));
        gk.addUpgradeable(address(ver));

        zks.transferMastership(address(gk));
        gk.addUpgradeable(address(zks));

        emit Addresses(address(gov), address(zks), address(ver), address(gk));

        finalizeGovernance(Governance(address(gov)), _validator, _governor);
    }

    function finalizeGovernance(Governance _governance, address _validator, address _finalGovernor) internal {
        address[] memory tokens = getTokens();
        for (uint i = 0; i < tokens.length; ++i) {
            _governance.addToken(tokens[i]);
        }
        _governance.setValidator(_validator, true);
        _governance.changeGovernor(_finalGovernor);
    }
}
