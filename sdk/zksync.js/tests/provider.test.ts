import { expect } from 'chai';
import { Provider } from '../src/provider';
import { getTokens } from 'reading-tool';

describe('Provider tests', function () {
    it('Update token set', async function () {
        const key = new Uint8Array(new Array(32).fill(5));
        const mainnetTokens = getTokens('mainnet');

        const tokens = mainnetTokens.slice(0, 1);

        const provider = await Provider.newMockProvider('mainnet', key, () => [...tokens]);

        tokens.push(mainnetTokens[1]);
        await provider.updateTokenSet();

        for (const token of tokens) {
            const resolvedToken = {
                symbol: provider.tokenSet.resolveTokenSymbol(token.symbol),
                decimals: provider.tokenSet.resolveTokenDecimals(token.symbol),
                address: provider.tokenSet.resolveTokenAddress(token.symbol),
                // name is not stored in tokenSet, so we just have to copy it
                name: token.name
            };
            expect(resolvedToken).to.eql(token, 'Token set has not been updated');
        }
    });
});
