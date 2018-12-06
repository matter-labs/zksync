var Migrations = artifacts.require("./Migrations.sol");
var Plasma = artifacts.require("./Plasma.sol");

module.exports = async function(deployer) {
    let from = "0x8b520CDbDE39c376cd8349E91F27A1b426DD3eA1"
    let to =   "0xe5d0efb4756bd5cdd4b5140d3d2e08ca7e6cf644"
    await web3.eth.sendTransaction({from: from, to: to, value: 1000000000});
    // let m = await deployer.deploy(Migrations);
    let plasma = await deployer.deploy(Plasma);
};
