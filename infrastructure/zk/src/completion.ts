// This file is responsible for generating shell completion
// If the completion script is installed, there is no need to update it after adding a new subcommand

import { Command, Option } from 'commander';
import tabtab from 'tabtab';

type CommandInfo = {
    command: string;
    description: string;
    options: string[];
    subcommands: CommandInfo[];
};

function commandInfo(cmd: Command): CommandInfo {
    return {
        command: cmd._name,
        description: cmd._description,
        options: cmd.options.map((option: Option) => option.long || option.short),
        subcommands: cmd.commands.map((subcmd) => commandInfo(subcmd as Command))
    };
}

// this actually completes the current subcommand
// see `tabtab` docs for a better understanding
function completer(env: any, info: CommandInfo) {
    if (!env.complete) return;
    if (env.prev == info.command) {
        tabtab.log(
            info.subcommands.map((subcmd) => {
                return {
                    name: subcmd.command,
                    description: subcmd.description
                };
            })
        );
        tabtab.log(info.options);
        return;
    }
    info.subcommands.map((subcmd) => completer(env, subcmd));
}

// the command `completion` without subcommands
// is only meant to be used internally by the completion
// engine of the shell and not by actual users.
export function command(program: Command) {
    // prettier-ignore
    const completion = new Command('completion')
        .description('generate shell completion scripts')
        .action(() => {
            const env = tabtab.parseEnv(process.env);
            const info = commandInfo(program);
            return completer(env, info);
        });

    completion
        .command('install')
        .description('install shell completions for zk')
        .action(async () => {
            await tabtab.install({
                name: 'zk',
                completer: 'zk'
            });
        });

    completion
        .command('uninstall')
        .description('uninstall shell completions for zk')
        .action(async () => {
            await tabtab.uninstall({ name: 'zk' });
        });

    return completion;
}
