import * as fs from 'fs';
import Axios from 'axios';

const apiTypesFolder = './api-types';

async function validateTypeJSON(typeFilePath: string, json: string) {
    const tmpFilePath = typeFilePath.replace(
        /\.ts$/, 
        `${new Date().toString().slice(16, 24).replace(/\:/g, '_')}.gen.ts`
    );
    const typeContent = fs.readFileSync(typeFilePath, 'utf-8');

    fs.writeFileSync(
        tmpFilePath,
        typeContent + `\n\nexport const val: Interface = ` + json + ';\n'
    );

    try {
        require(tmpFilePath);
        fs.unlinkSync(tmpFilePath);
    } catch (e) {
        console.error(`Error in type ${typeFilePath}:`)
        console.error(e.message.split('\n').slice(2, 7).join('\n'));
        console.error(`Check file ${tmpFilePath} to see the error`)

        throw new Error(`Rest api response format error.`);
    }
}

export async function checkStatus() {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/status`;
    const typeFilePath = `${apiTypesFolder}/status.ts`;

    const { data } = await Axios.get(url);
    const serverJson = JSON.stringify(data, null, 4);

    await validateTypeJSON(typeFilePath, serverJson);

    // additional checks
}

export async function checkAccount(address: string) {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/account/${address}`;
    const typeFilePath = `${apiTypesFolder}/account.ts`;

    const { data } = await Axios.get(url);
    const serverJson = JSON.stringify(data, null, 4);

    await validateTypeJSON(typeFilePath, serverJson);

    // additional checks
}

export async function checkTxHistory(address: string) {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/account/${address}/history/0/20`;
    const typeFilePath = `${apiTypesFolder}/tx-history.ts`;

    const { data } = await Axios.get(url);
    const serverJson = JSON.stringify(data, null, 4);

    await validateTypeJSON(typeFilePath, serverJson);

    // additional checks
}

export function deleteUnusedGenFiles() {
    fs.readdirSync(apiTypesFolder)
        .filter(n => n.endsWith('.gen.ts'))
        .map(n => apiTypesFolder + '/' + n)
        .forEach(fs.unlinkSync);
}
