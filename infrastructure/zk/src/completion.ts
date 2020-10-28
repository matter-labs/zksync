import { Command, Option } from 'commander';
import tabtab from 'tabtab';

type CommandInfo = {
    command: string;
    description: string;
    options: string[];
    subcommands: CommandInfo[];
}

function commandInfo(cmd: Command): CommandInfo {
    return {
        command: cmd._name,
        description: cmd._description,
        options: cmd.options.map((option: Option) => option.long || option.short),
        subcommands: cmd.commands.map((subcmd) => commandInfo(subcmd as Command))
    };
}

function completion(env: any, info: CommandInfo) {
    if (!env.complete) return;
    if (env.prev == info.command) {
        tabtab.log(info.subcommands.map(subcmd => subcmd.command));
        tabtab.log(info.options);
        return;
    }
    info.subcommands.map(subcmd => completion(env, subcmd));
}

export function command(program: Command) {
    const complete = new Command('completion')
        .description('generate shell completion scripts')
        .action(() => {
            const env = tabtab.parseEnv(process.env);
            const info = commandInfo(program);
            return completion(env, info);
        });

    complete
        .command('install')
        .description('install shell completions for zk')
        .action(async () => {
            await tabtab.install({ 
                name: 'zk',
                completer: 'zk'
            });
        });

    complete
        .command('uninstall')
        .description('uninstall shell completions for zk')
        .action(async () => {
            await tabtab.uninstall({ name: 'zk' });
        });

    return complete;
}
