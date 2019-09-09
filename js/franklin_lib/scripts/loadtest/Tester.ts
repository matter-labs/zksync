import { Token } from '../../src/wallet';
import { LocalWallet } from "./LocalWallet";
import { ReceiveMoneyOperation } from './ReceiveMoneyOperation';
import { DepositOperation } from './DepositOperation';
import { WithdrawOperation } from './WithdrawOperation';
import { TransferOperation } from './TransferOperation';
import { bigNumberify, BigNumber } from "ethers/utils";
import Prando from 'prando';
import { AbstractOperation } from './AbstractOperation';
const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms))

interface TesterKwargs {
    initNumWallets: number,
    randomSeed: string | number | null
}
export class Tester {
    wallets: LocalWallet[] = [];
    prando: Prando;
    tokens: Token[];

    private constructor() {}
    
    public static async new(kwargs: TesterKwargs): Promise<Tester> {
        const tester: Tester = new Tester();        
        tester.prando = new Prando(kwargs.randomSeed);
        for (let i = 0; i < kwargs.initNumWallets; i++) {
            await tester.addNewWallet();
        }
        tester.tokens = tester.wallets[0].franklinWallet.supportedTokens;
        return tester;
    }

    async addNewWallet(): Promise<void> {
        this.wallets.push( await LocalWallet.new() );
    }
    
    private selectRandomWallet(): LocalWallet {
        return this.prando.nextArrayItem(this.wallets);
    }
    
    private selectAnotherRandomWallet(wallet: LocalWallet): LocalWallet {
        if (this.wallets.length < 2) throw new Error('there is no two wallets.');
        do {
            var wallet2 = this.selectRandomWallet();
        } while (wallet === wallet2);
        return wallet2;
    }
    
    private selectTwoRandomDistinctWallets() {
        let w1 = this.selectRandomWallet();
        let w2 = this.selectAnotherRandomWallet(w1);
        return [w1, w2];
    }
    
    private selectRandomAmount(from: number, to: number): BigNumber {
        // TODO
        return bigNumberify(this.prando.nextBoolean() ? from : to);
        // return bigNumberify(prando.nextInt(from, to).toString());
    }
    
    private selectRandomToken(): Token {
        return this.prando.nextArrayItem(this.tokens);
    }

    randomReceiveMoneyOperation(kwargs): ReceiveMoneyOperation {
        kwargs = kwargs || {};
        kwargs.wallet = kwargs.wallet || this.selectRandomWallet();
        kwargs.token  = kwargs.token  || this.selectRandomToken();
        kwargs.amount = kwargs.amount || 
            (kwargs.token.id == 0 
            ? bigNumberify('10000000000000000') 
            : bigNumberify('1000000'));
        return new ReceiveMoneyOperation(kwargs);
    }

    randomDepositOperation(kwargs): DepositOperation {
        kwargs = kwargs || {};
        kwargs.wallet = kwargs.wallet || this.selectRandomWallet();
        kwargs.token  = kwargs.token  || this.selectRandomToken();
        kwargs.amount = kwargs.amount || this.selectRandomAmount(0, 1000);
        kwargs.fee    = kwargs.fee    || this.selectRandomAmount(0, 1000);
        return new DepositOperation(kwargs);
    }

    randomWithdrawOperation(kwargs): WithdrawOperation {
        kwargs = kwargs || {};
        kwargs.wallet = kwargs.wallet || this.selectRandomWallet();
        kwargs.token  = kwargs.token  || this.selectRandomToken();
        kwargs.amount = kwargs.amount || this.selectRandomAmount(0, 1000);
        kwargs.fee    = kwargs.fee    || this.selectRandomAmount(0, 1000);
        return new WithdrawOperation(kwargs);
    }

    randomTransferOperation(kwargs): TransferOperation {
        kwargs = kwargs || {};
        kwargs.wallet1 = kwargs.wallet1 || this.selectRandomWallet();
        kwargs.wallet2 = kwargs.wallet2 || this.selectAnotherRandomWallet(kwargs.wallet1);
        kwargs.token   = kwargs.token   || this.selectRandomToken();
        kwargs.amount  = kwargs.amount  || this.selectRandomAmount(10, 1000);
        kwargs.fee     = kwargs.fee     || this.selectRandomAmount(10, 1000);
        return new TransferOperation(kwargs);
    }

    /**
     * it creates one of the available operations.
     * The operation should be passed to addOperation() method.
     * 
     */
    randomOperation(): AbstractOperation {
        const opsProbs = {
            'randomReceiveMoneyOperation': 0.3,
            'randomDepositOperation': 0.3,
            'randomTransferOperation': 0.4
        };
        let num = this.prando.next();
        let keys = Object.keys(opsProbs);
        for (let i = 0; i < keys.length; i++) {
            let opName = keys[i];
            let prob = opsProbs[opName];
            if (num < prob) {
                return this[opName]();
            }
            num -= prob;
        }
    }

    /**
     * adds some random operation to one of the wallets[].
     * @param op 
     */
    addOperation(op: AbstractOperation) {
        op = op || this.randomOperation();
        op.mainWallet.addAction(op);
    }

    async run(): Promise<void> {
        await Promise.all(this.wallets.map(async wallet => {
            for (let i = 0; i < wallet.actions.length; i++) {
                const action = wallet.actions[i];
                await action.start();
                console.log(action.humanReadableLogs());
            }
        }))
    
        await sleep(5000);
    
        await Promise.all(this.wallets.map(async wallet => {
            console.log(`\n\n${await wallet.getWalletDescriptionString()}`);
        }));
    }

    /**
     * returns json string of every operation on every wallet.
     */
    public async dump(): Promise<string> {
        return '[' + (await Promise.all(this.wallets.map(async w => w.toJSON()))).join(', ') + ']';
    }
}
