import { exec } from 'child_process';
import { promisify } from 'util';

// async executor of shell commands
export const sh = promisify(exec);
