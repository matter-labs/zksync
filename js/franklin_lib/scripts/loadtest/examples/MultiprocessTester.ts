import { fork, ChildProcess } from 'child_process';

function join(process: ChildProcess): Promise<number> {
    return new Promise(resolve => {
        process.on('exit', resolve);
    });
}

async function main() {
    let numWallets   = Number(process.argv[2]);
    let numProcesses = Number(process.argv[3]);
    let runGoal      = String(process.argv[4]);

    let numWalletsInShard = numWallets / numProcesses;

    let exitCode = -1;

    if (runGoal.includes('replenish')) {
        let replenish = fork('scripts/loadtest/examples/loadtest.ts', [numWallets, 0, 0, 'replenish'].map(String));
        exitCode = await join(replenish);
    }

    if (runGoal.includes('runTest')) {
        let processes = [];
        for (let i = 0; i < numProcesses; ++i) {
            let args = [numWallets, i * numWalletsInShard, numWalletsInShard, 'run the actual test'].map(String);
            processes.push(fork('scripts/loadtest/examples/loadtest.ts', args));
            // processes.push(fork('scripts/loadtest/examples/lightweight_loadtest.ts', args));
        }
    
        let exitCodes = await Promise.all(processes.map(join));
        exitCode = Math.max(...exitCodes);
    }

    process.exit(exitCode);
}

main()
