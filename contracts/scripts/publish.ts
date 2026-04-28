const hre = require('hardhat');

async function main() {
    if (process.env.CHAIN_ETH_NETWORK == 'localhost') {
        console.log('Skip contract publish on localhost');
        return;
    }
    for (const address of ['0x57B09100e6160503aBDEBC76012b6c358eA4e462']) {
        try {
            await hre.run('verify:verify', { address });
        } catch (e) {
            console.error(e);
        }
    }
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
