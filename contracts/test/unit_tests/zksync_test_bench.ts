import {ethers} from 'ethers';
import {parseEther} from 'ethers/lib/utils';
import {ETHProxy} from 'zksync';
import {Deployer, readContractCode, readProductionContracts} from '../../src.ts/deploy';
import {ZkSyncProcessOpUnitTest, ZkSyncProcessOpUnitTestFactory} from '../../typechain';

const hardhat = require('hardhat');
const {simpleEncode} = require('ethereumjs-abi');
const {expect} = require('chai');
const {getCallRevertReason, IERC20_INTERFACE} = require('./common');

const TEST_PRIORITY_EXPIRATION = 101;
const CHUNK_SIZE = 9;

let wallet, exitWallet;


describe('ZK priority queue ops unit tests', function () {
    this.timeout(50000);

    let zksyncContract: ZkSyncProcessOpUnitTest;
    let tokenContract;
    let ethProxy;
    before(async () => {
        [wallet, exitWallet] = await hardhat.ethers.getSigners();

        const contracts = readProductionContracts();
        contracts.zkSync = readContractCode("dev-contracts/ZkSyncProcessOpUnitTest");
        const deployer = new Deployer({deployWallet: wallet, contracts});
        await deployer.deployAll({gasLimit: 6500000});
        zksyncContract = ZkSyncProcessOpUnitTestFactory.connect(deployer.addresses.ZkSync, wallet);

        const tokenContractFactory = await hardhat.ethers.getContractFactory('TestnetERC20Token');
        tokenContract = await tokenContractFactory.deploy('Matter Labs Trial Token', 'MLTT', 18);
        await tokenContract.mint(wallet.address, parseEther('1000000'));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);

        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: govContract.address
        });
    });

    it('change pubkey measure gas', async () => {
        const CHUNK_SIZE = 9;
        const pubdata = ethers.utils.concat([]);
        const ethWitness = ethers.utils.concat([]);
        const processableOperationsHash = ethers.utils.keccak256(pubdata);
        const offsetCommitment = new Uint8Array(pubdata.length/CHUNK_SIZE);
        offsetCommitment[0] = 1;
        await zksyncContract.collectOnchainOpsExternal({
                blockNumber: 0,
                feeAccount: 0,
                newStateHash: ethers.constants.HashZero,
                publicData: pubdata,
                timestamp: 0,
                onchainOperations: [{publicDataOffset: 0, ethWitness}],
            },
            processableOperationsHash,
            0,
            offsetCommitment
        );
    });
});
