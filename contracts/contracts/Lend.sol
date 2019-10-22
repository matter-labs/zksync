pragma solidity ^0.5.8;

import "openzeppelin-solidity/contracts/token/ERC20/IERC20.sol";

import "./Governance.sol";
import "./Franklin.sol";
import "./Verifier.sol";

contract Lending {

    Verifier internal verifier;
    Governance internal governance;
    Franklin internal franklin;

    constructor(
        address _governanceAddress,
        address _franklinAddress,
        address _verifierAddress
    ) public {
        governance = Governance(_governanceAddress);
        franklin = Franklin(_priorityQueueAddress);
        verifier = Verifier(_verifierAddress);
    }

    function requestLoan(
        bytes32 _message,
        bytes32 _signature,
        bytes32 _txHash,
        uint32 _blockNumber,
        address _borrower,
        address _reciever,
        uint16 _tokenId,
        uint128 _amount
    ) external {
        require(
            verifier.verifySignature(_message, _signature),
            "lrn11"
        ); // lrn11 - verification failed - wrong signature
        require(
            verifier.verifyTx(_txHash, _blockNumber, _sender, _tokenId, _amount),
            "lrn12"
        ); // lrn12 - verification failed - wrong tx

        loanRequests[loanRequestsCount] = LoanRequest({
            blockNumber: _blockNumber, // when block comes - search tx and update value
            borrower: _borrower,
            lender: address(0),
            receiver: _reciever,
            token: _tokenId,
            amount: _amount
        });

        emit LoanRequested(
            loanRequestsCount,
            _blockNumber,
            _reciever,
            _tokenId,
            _amount
        );

        loanRequestsCount++;
    }
}