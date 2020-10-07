import * as ethers from 'ethers';
import * as zksync from 'zksync';

type Network = 'localhost' | 'rinkeby' | 'ropsten';

export class Tester {

    constructor(
        public network: Network,
        public ethProvider: ethers.providers.Provider,
        public syncProvider: zksync.Provider,
        public ethWallet: ethers.Wallet,
        public syncWallet: zksync.Wallet
    ) {}

    static async init(network: Network, transport: 'WS' | 'HTTP') {
        // prettier-ignore
        const ethProvider = network == 'localhost' 
            ? new ethers.providers.JsonRpcProvider() 
            : ethers.getDefaultProvider(network);
        const syncProvider = await zksync.getDefaultProvider(network, transport);
        const ethWallet = ethers.Wallet.fromMnemonic(
            process.env.TEST_MNEMONIC as string,
            "m/44'/60'/0'/0/0"
        ).connect(ethProvider);
        const syncWallet = await zksync.Wallet.fromEthSigner(ethWallet, syncProvider);
        return new Tester(network, ethProvider, syncProvider, ethWallet, syncWallet);
    }

    async disconnect() { await this.syncProvider.disconnect(); }

    async fundedWallet(amount: string) {
        const newWallet = ethers.Wallet.createRandom().connect(this.ethProvider);
        const syncWallet = await zksync.Wallet.fromEthSigner(newWallet, this.syncProvider);
        const handle = await this.ethWallet.sendTransaction({
            to: newWallet.address, 
            value: ethers.utils.parseEther(amount)
        });
        await handle.wait();
        return syncWallet;
    }

    async emptyWallet() {
        let ethWallet = ethers.Wallet.createRandom().connect(this.ethProvider);
        return await zksync.Wallet.fromEthSigner(ethWallet, this.syncProvider);
    }
}

