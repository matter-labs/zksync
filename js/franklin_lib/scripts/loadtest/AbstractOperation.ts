import { LocalWallet } from './LocalWallet';
interface AbstractOperationLog {
    operationId: string,
    start: string,
    logs: string[],
    finish: string,
    success: boolean,
    balanceAtStart: string[],
    balanceAtEnd: string[]
}
export abstract class AbstractOperation {
    private static getOperationId = (counter => () => String(++counter).padStart(3, '0'))(0);
    private info: AbstractOperationLog;
    
    public constructor(public mainWallet: LocalWallet) {
        this.info = {
            operationId: `${mainWallet.franklinWallet.address.toString('hex')} ${AbstractOperation.getOperationId()}`,
            start: null,
            logs: [],
            finish: null,
            success: null,
            balanceAtStart: null,
            balanceAtEnd: null
        };
    }

    protected logStart(msg: string) {
        this.info.start = msg;
    }
    protected logFinish(msg: string) {
        this.info.finish = msg;
    }
    protected log(msg: string) {
        this.info.logs.push(msg);
    }
    public logsJSON(): string {
        return JSON.stringify(this.info, null, 4);
    }
    public static humanReadableLogsFromJSON(json: string): string {
        let info: AbstractOperationLog = JSON.parse(json);
        
        let logs = info.logs;
        logs.unshift(info.start);
        logs.push(info.finish);

        return logs.map(l => `${info.operationId} ${l}`).join('\n');
    }
    public humanReadableLogs(): string {
        return AbstractOperation.humanReadableLogsFromJSON(this.logsJSON());
    }

    protected abstract async action(): Promise<void>;
    protected abstract kwargs: any;
    public async start() {
        try {
            this.info.balanceAtStart = await this.mainWallet.getAllBalancesString();
            await this.action();
            this.log(`succeeded`);
            this.info.success = true;
        } catch (err) {
            this.info.success = false;
            this.mainWallet.resetNonce();
            this.log(`failed with ${err.message}`);
        }
        let balanceStrings = await this.mainWallet.getBalanceForTokenAsString(this.kwargs.token);
        balanceStrings.forEach(this.log.bind(this));
        this.info.balanceAtEnd = balanceStrings;
        this.logFinish(`finished.`);
    }
}
