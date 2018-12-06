/*
 * NB: since truffle-hdwallet-provider 0.0.5 you must wrap HDWallet providers in a 
 * function when declaring them. Failure to do so will cause commands to hang. ex:
 * ```
 * mainnet: {
 *     provider: function() { 
 *       return new HDWalletProvider(mnemonic, 'https://mainnet.infura.io/<infura-key>') 
 *     },
 *     network_id: '1',
 *     gas: 4500000,
 *     gasPrice: 10000000000,
 *   },
 */

var HDWalletProvider = require("truffle-hdwallet-provider");

module.exports = {
  // See <http://truffleframework.com/docs/advanced/configuration>
  // to customize your Truffle configuration!
    compilers: {
       solc: {
         version: "0.4.24" // ex:  "0.4.20". (Default: Truffle's installed solc)
       }
    },

    networks: {
        // truffle test --network dev
        dev: {
          host: "127.0.0.1",
          port: 8545,
          network_id: "*" // match any network
        },

      rinkeby:{
        network_id: 4,
        provider: function() { 
          let url = `https://rinkeby.infura.io/${process.env.INFURA_PROJECT_ID}`
          let mnemonic = process.env.MNEMONIC
          return new HDWalletProvider(mnemonic, url) 
        },
      },

      ropsten:{
        network_id: 3,
        provider: function() { 
          let url = `https://ropsten.infura.io/${process.env.INFURA_PROJECT_ID}`
          let mnemonic = process.env.MNEMONIC
          return new HDWalletProvider(mnemonic, url) 
        },
      },

    }
};
