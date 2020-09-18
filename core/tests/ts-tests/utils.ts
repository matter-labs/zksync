const ethers = require('ethers');
const zksync = require('zksync');
const readline = require("readline");

export function isPowerOfTwo(n: number): boolean {
    return (n & (n - 1)) === 0; 
}

export function * product(...arrays: any[][]) {
    if (arrays.length === 0) {
        yield [];
        return;
    };
    const [head, ...tails] = arrays;
    for (const elem of head) {
        for (const elems of product(...tails)) {
            yield [elem, ...elems];
        }
    }
}

export const insufficientFundsHandler = e => {
    if (e.message.includes('insufficient funds')) {
        return {
            reason: 'insufficient funds',
        };
    }

    throw e;
}

export function jrpcErrorHandler(message) {
    return error => {
        if (error.jrpcError) {
            return {
                error: error.jrpcError.message
            };
        } else {
            error.message = `${message}: ${error.message}`; // we change the passed object, uh-oh
            throw error;
        }
    }
}

export function logAndReturn(arg) {
    console.log(arg);
    return arg;
}

export const logAndThrow = msg => arg => {
    console.log(msg, ': ', arg);
    throw arg;
}

export function splitAmount(totalAmount, feeDivisor = 100) {
    const fee = totalAmount.div(feeDivisor);
    const amount = totalAmount.sub(fee);
    return [amount, fee];
}

export function * range(start, end?) {
    if (end == undefined) {
        [start, end] = [0, start];
    }
    
    for ( ; start < end; ++start) {
        yield start;
    }

    return;
}

export function rangearr(start, end?) {
    return [...range(start, end)];
}

export const flat = arr => Array.isArray(arr) ? [].concat(...arr.map(flat)) : arr;

export function input(questionText) {
    const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout
    });

    return new Promise(resolve => {
        rl.question(questionText, function(answer) {
            rl.close();
            resolve(answer);
        });
    });
}
