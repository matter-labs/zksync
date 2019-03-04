var Migrations = artifacts.require("./Migrations.sol");
var PlasmaContract = artifacts.require("./PlasmaContract.sol");
var Exitor = artifacts.require("./PlasmaExitor.sol");
var Transactor = artifacts.require("./PlasmaTransactor.sol");

module.exports = async function(deployer) {
    let m = await deployer.deploy(Migrations);

    await deployer.deploy(Exitor);
    let ex = await Exitor.deployed();

    await deployer.deploy(Transactor);
    let tr = await Transactor.deployed();

    let paddingPubKey = JSON.parse(process.env.PADDING_PUB_KEY);
    let plasma = await deployer.deploy(PlasmaContract, tr.address, ex.address, paddingPubKey);
};
