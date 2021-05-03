import { Command } from 'commander';
import * as utils from '../utils';

async function run_event_listener() {
    let child = utils.background('cargo run --bin zksync_event_listener --release');

    // delegate processing of pressing `Ctrl + C`
    process.on('SIGINT', () => {
        child.kill('SIGINT');
    });
}

export const command = new Command('event-listener').description('start zkSync event-listener');
command.action(run_event_listener);
