import * as fs from 'fs';
import * as path from 'path';
import * as Handlebars from 'handlebars';

const network = process.env.ETH_NETWORK as string;
const ZKSYNC_HOME = process.env.ZKSYNC_HOME as string;
if (!network) {
    throw new Error('ETH_NETWORK is not set');
}
if (!ZKSYNC_HOME) {
    throw new Error('ZKSYNC_HOME is not set');
}

const pathToTokenList = path.join(ZKSYNC_HOME, `etc/tokens/${network}.json`);
const tokenList = JSON.parse(fs.readFileSync(pathToTokenList, { encoding: 'utf-8' }));
const pathToExample = path.join(ZKSYNC_HOME, `contracts/contract-templates/TokenInitTemplate.template`);
const template = Handlebars.compile(fs.readFileSync(pathToExample, { encoding: 'utf-8' }));
const templateParams = {
    token_len: tokenList.length,
    tokens: tokenList
};
const contract = template(templateParams);
const outputPath = path.join(ZKSYNC_HOME, `contracts/contracts/TokenInit.sol`);
fs.writeFileSync(outputPath, contract, { encoding: 'utf-8' });
