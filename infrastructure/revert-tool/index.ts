import * as ethers from 'ethers';

const ethProvider = ethers.getDefaultProvider('http://localhost:8545');
let ethWallet = ethers.Wallet.fromMnemonic("stuff slice staff easily soup parent arm payment cotton trade scatter struggle", "m/44'/60'/0'/0/0")
.connect(ethProvider);
//ethWallet.getBalance().then((res)=>console.log(res));
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
contract.es
contract.revertBlocks(
    [
      {
        blockNumber:5, 
        priorityOperations:0, 
        pendingOnchainOperationsHash:'0x54d3752cc13ebb42f1568335b4a570e90824dce071fe737349476ae5dc04e387', 
        timestamp:1613473015, 
        stateHash:'0x23ccbb3d0225b91ef50975c592f9b9af562440cca4770c8372d6135fd38057dd', 
        commitment:'0xe4da5fc3e1fbd9b02b6c5a38b7320e0156ed5d943234410a4be6623ad66bea7b'
      }
    ],
    {
      gasLimit:'10000000' 
    }
  )
    .then(res => {
        console.log(res);
    })
    .catch(err => {
        console.log('aaa');
        console.log(err);
    })
//console.log(contract);