import Prando from 'prando';
import { ethers } from 'ethers';
import { parseEther } from 'ethers/utils';
import { Wallet, Token, Address, FranklinProvider } from '../../../src/wallet';

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const franklinProvider = new FranklinProvider();

class LightWallet {
    public fraAddress: string;
    
    private constructor(
        public id: number, 
        public wallet: ethers.Wallet, 
        public franklinWallet: Wallet, 
    ) {
        this.fraAddress = this.franklinWallet.address.toString();
    }

    public static async new(id: number) {
        let ethWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/3/" + id).connect(provider);
        let fraWallet = new LightWallet(id, ethWallet, await Wallet.fromEthWallet(ethWallet));
        return fraWallet;
    }

    public async deposit(token, amount, fee) {
        let handle = await this.franklinWallet.deposit(token, amount, fee);
        handle.waitCommit();
    }

    public async transfer(toFraAddress, token, amount, fee) {
        let handle = await this.franklinWallet.transfer(toFraAddress, token, amount, fee);
        // await handle.waitCommit();
    }
}


async function test() {
    let initNumWallets    = Number(process.argv[2]);
    let shardWalletOffset = Number(process.argv[3]);
    let shardWalletLimit  = Number(process.argv[4]);

    let range = Array.from(Array(initNumWallets).keys()); // like python range
    let wallets = await Promise.all(range.map(LightWallet.new));

    let prando = new Prando();

    let shardWallets = wallets.slice(shardWalletOffset, shardWalletLimit);

    // await Promise.all(shardWallets.map(async wallet => {
    //     await wallet.deposit(0, parseEther('100'), parseEther('0.01'));
    //     await wallet.deposit(1, parseEther('100'), parseEther('0.05'));
    // }));

    let testStartTime = Date.now();
    console.log('Begin transactions');
    await Promise.all(shardWallets.map(async wallet => {
        for (let i = 0; i < 100; ++i) {
            let otherWallet = prando.nextArrayItem(wallets);
            await wallet.transfer(otherWallet.fraAddress, 0, parseEther('0.001'), 0);
        }
    }));
    console.log('Finished test in', Date.now() - testStartTime, 'ms');
}

test()
