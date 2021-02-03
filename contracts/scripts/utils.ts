import { ethers } from 'ethers';

export function web3Url() {
    return process.env.ETH_CLIENT_WEB3_URL.split(',')[0] as string;
}

export function web3Provider() {
    return new ethers.providers.JsonRpcProvider(web3Url());
}
