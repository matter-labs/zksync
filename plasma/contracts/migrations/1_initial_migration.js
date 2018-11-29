var Migrations = artifacts.require("./Migrations.sol");
var Plasma = artifacts.require("./Plasma.sol");

module.exports = async function(deployer) {
    await deployer.deploy(Migrations);
    await deployer.deploy(Plasma);
};
