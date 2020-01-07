import ethers = require('ethers');
const zksync = require('zksync');

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

export function jrpcErrorHandler(message) {
    return error => {
        if (error.jrpcError) {
            throw new Error(`${message}: ${error.jrpcError.message}`);
        } else {
            error.message = `${message}: ${error.message}`; // we change the passed object, uh-oh
            throw error;
        }
    }
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
