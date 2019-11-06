// Post contract abi to tesseracts.
import Axios from 'axios';
const FormData = require('form-data');
const qs = require('querystring');

const franklinContractCode = require('../build/Franklin');

async function main() {
    let req = {
        contract_source: JSON.stringify(franklinContractCode.abi),
        contract_compiler: "abi-only",
        contract_name: "Franklin",
        contract_optimized: false
    };

    const config = {
        headers: {
            'Content-Type': 'application/x-www-form-urlencoded'
        }
    };
    await Axios.post(`${process.env.TESSERACTS_URL}/0xc56e79caa94c96de01ef36560ac215cc7a4f0f47/contract`, qs.stringify(req), config);
}

main();
