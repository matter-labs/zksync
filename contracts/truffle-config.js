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

module.exports = {
  // See <http://truffleframework.com/docs/advanced/configuration>
  // to customize your Truffle configuration!
    compilers: {
       solc: {
         version: "0.4.24" // ex:  "0.4.20". (Default: Truffle's installed solc)
       }
    },

    networks: {

      universal: {
        network_id: '9',
        gas: 6900000,
        provider: function() { 
          const HDWalletProvider = require("truffle-hdwallet-provider");
          let url = `${process.env.WEB3_URL}`
          let mnemonic = process.env.MNEMONIC
          return new HDWalletProvider(mnemonic, url) 
        },
      },

      mainnet: {
        network_id: '1',
        gas: 6900000,
        provider: function() { 
          const HDWalletProvider = require("truffle-hdwallet-provider");
          let url = `${process.env.WEB3_URL}`
          let mnemonic = process.env.MNEMONIC
          return new HDWalletProvider(mnemonic, url) 
        },
      },

      rinkeby: {
        network_id: '4',
        gas: 6900000,
        provider: function() { 
          const HDWalletProvider = require("truffle-hdwallet-provider");
          let url = `${process.env.WEB3_URL}`
          let mnemonic = process.env.MNEMONIC
          return new HDWalletProvider(mnemonic, url) 
        },
      },


      // rinkeby: {
      //   network_id: 4,
      //   gas: 6900000,
      //   provider: function() { 
      //     const HDWalletProvider = require("truffle-hdwallet-provider");
      //     //let url = `https://rinkeby.infura.io/v3/${process.env.INFURA_PROJECT_ID}`
      //     let url = `${process.env.WEB3_URL}`
      //     let mnemonic = process.env.MNEMONIC
      //     return new HDWalletProvider(mnemonic, url) 
      //   },
      // },

    }
};
