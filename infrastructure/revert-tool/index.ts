import * as ethers from 'ethers';

const ethProvider = ethers.getDefaultProvider('http://localhost:8545');
let ethWallet = ethers.Wallet.createRandom().connect(ethProvider);

const ABI = [
    {
        "inputs": [
          {
            "components": [
              {
                "internalType": "uint32",
                "name": "blockNumber",
                "type": "uint32"
              },
              {
                "internalType": "uint64",
                "name": "priorityOperations",
                "type": "uint64"
              },
              {
                "internalType": "bytes32",
                "name": "pendingOnchainOperationsHash",
                "type": "bytes32"
              },
              {
                "internalType": "uint256",
                "name": "timestamp",
                "type": "uint256"
              },
              {
                "internalType": "bytes32",
                "name": "stateHash",
                "type": "bytes32"
              },
              {
                "internalType": "bytes32",
                "name": "commitment",
                "type": "bytes32"
              }
            ],
            "internalType": "struct Storage.StoredBlockInfo[]",
            "name": "_blocksToRevert",
            "type": "tuple[]"
          }
        ],
        "name": "revertBlocks",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
      },
];

const contract = new ethers.Contract('0x0f96cf4fa973c42c75c39813f447ae2f10e57893', ABI, ethWallet);
contract.revertBlocks([{
    blockNumber:2, 
    priorityOperations:0, 
    pendingOnchainOperationsHash:'0x7c7144eace54bb0ee39e64a18453dd527692614facf412491310e6bd323765c5', 
    timestamp:1613060560, 
    stateHash:'0x05d27037d49afa0cd7f20f1e503f38e5d569d6645421ae72c51c6d3dd3f43bdc', 
    commitment:'0x8a3ec775c9e8022967ad64b2c025cb487ca8f03dc37ae2d04748e2b2fc7c3e6c'}
    ])
    .then(res => {
        console.log(res);
    })
    .catch(err => {
        console.log('aaa');
        console.log(err);
    })
//console.log(contract);