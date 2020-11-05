import "@nomiclabs/hardhat-waffle";
import "hardhat-contract-sizer";
import "@nomiclabs/hardhat-solpp";
import "hardhat-typechain"

export default {
  solidity: {
    version: "0.7.3",
    settings: {
      optimizer: {
        enabled: true,
        runs: 200,
      },
    },
  },
  contractSizer: {
    runOnCompile: true,
  },
  paths: {
      sources: "./contracts"
  },
  solpp: {
      defs: {
        UPGRADE_NOTICE_PERIOD: 0,
        MAX_AMOUNT_OF_REGISTERED_TOKENS: 5,
      },
  },
};
