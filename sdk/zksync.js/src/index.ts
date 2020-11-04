import { Wallet } from './wallet';
import { Provider, ETHProxy, getDefaultProvider } from './provider';
import { Signer } from './signer';
import { closestPackableTransactionAmount, closestPackableTransactionFee } from './utils';

import * as wallet from './wallet';
import * as types from './types';
import * as utils from './utils';
import * as crypto from './crypto';

export {
    Wallet,
    Signer,
    Provider,
    ETHProxy,
    closestPackableTransactionFee,
    closestPackableTransactionAmount,
    getDefaultProvider,
    types,
    utils,
    crypto,
    wallet
};
