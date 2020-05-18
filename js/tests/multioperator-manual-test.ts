import * as zksync from 'zksync';
import * as ethers from 'ethers';
import { bigNumberify, parseEther, formatEther } from 'ethers/utils';
import { Deployer } from '../../contracts/src.ts/deploy';
import { input } from './utils';

let ethersProvider: ethers.providers.Provider;
let syncProvider: zksync.Provider;
let ethW1: ethers.Wallet;
let syncW1: zksync.Wallet;
let ethW2: ethers.Wallet;
let syncW2: zksync.Wallet;
let deployer: Deployer;

async function reconnectServer() {
    if (syncProvider) {
        await syncProvider.disconnect();
    }

    ethersProvider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    syncProvider = await zksync.Provider.newWebsocketProvider(process.env.WS_API_ADDR);

    //* ethW1, syncW1 is the validator wallet.
    //* The other ones are extra validators.
    [ 
        [ ethW1, syncW1 ], 
        [ ethW2, syncW2 ],
    ] = await Promise.all(
        [
            process.env.MNEMONIC,
            process.env.EXTRA_OPERATOR_MNEMONIC_1,
        ]
        .map(mnemonic => ethers.Wallet.fromMnemonic(mnemonic, "m/44'/60'/0'/0/1"))
        .map(ethW => ethW.connect(ethersProvider))
        .map(async ethW => (<[ethers.Wallet, zksync.Wallet]>[
            ethW,
            await zksync.Wallet.fromEthSigner(ethW, syncProvider)
        ]))
    );

    deployer = new Deployer({deployWallet: ethW1});
}

async function test() {
    console.log("Welcome to the manual testing Wizard!");
    console.log("Launch server and prover.");
    console.log();
    await input("When finished, press enter:");

    await reconnectServer();

    //* Make sure extra validator has enough funds.
    await ethW1.sendTransaction({ to: ethW2.address, value: parseEther('1.0') });

    //* Deposit for extra validator to add it to the account tree.
    for (const token of ['ETH'/* , process.env.TEST_ERC20 */]) {
        console.log('Balance of ' + token + ': ' + formatEther(await syncW2.getEthereumBalance(token)));
        const deposit = await syncW2.depositToSyncFromEthereum({
            depositTo: syncW2.address(),
            token,
            amount: parseEther("0.1"),
            approveDepositAmountForERC20: true,
        });
        await deposit.awaitReceipt();

        if (! await syncW2.isSigningKeySet()) {
            const changePubKey = await syncW2.setSigningKey();
            await changePubKey.awaitReceipt();
        }
    }

    console.log("Now add more txs, for example, by running");
    console.log("zksync integration-simple");
    console.log("Wait until enough blocks get verified.");
    console.log();
    await input("When finished, press enter:");

    console.log(`Now let's add extra validator to Governance.`);
    console.log("Copy these lines:");
    console.log();
    console.log(`MNEMONIC="${process.env.EXTRA_OPERATOR_MNEMONIC_1}"`);
    console.log(`OPERATOR_PRIVATE_KEY=${ethW2.privateKey.slice(2)}`);
    console.log(`OPERATOR_ETH_ADDRESS=${ethW2.address}`);
    console.log(`OPERATOR_FRANKLIN_ADDRESS=${ethW2.address}`);
    console.log();
    console.log("to the end of dev.env");

    await deployer.governanceContract(ethW1)
        .setValidator(ethW2.address, true)
        .then(tx => tx.wait());

    console.log();
    console.log("Turn off server, provers.");
    console.log();
    console.log("Then, run:");
    console.log("zksync data-restore-restart");
    console.log();
    console.log("When all blocks will be restored, relaunch server and prover.");
    console.log();
    await input("When finished, press enter:");

    await reconnectServer();

    //* now let's make a withdraw from operator account
    for (const token of ['ETH'/* , process.env.TEST_ERC20 */]) {
        const before = await syncW2.getBalance(token, "verified");
        
        const withdraw = await syncW2.withdrawFromSyncToEthereum({
            ethAddress: syncW2.address(),
            token,
            amount: parseEther("0.01"),
            fee: parseEther('0.0'),
        })
        
        await withdraw.awaitReceipt();
        console.log('withdraw hash:', withdraw.txHash);
        await withdraw.awaitVerifyReceipt();
        
        const after = await syncW2.getBalance(token, "verified");

        console.log(`zkSync balance before withdraw: ${before.toString()}`);
        console.log(`zkSync balance after withdraw: ${after.toString()}`);
    }

    console.log("So, how do you like new balance?");
    console.log("You may run zksync integration-simple again,");
    console.log("just to ensure everything works.");
    console.log()
    console.log("Congratulations, continuing running from another validator works!");

    await syncProvider.disconnect();
}

test();
