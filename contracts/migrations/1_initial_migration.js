var Migrations = artifacts.require("./Migrations.sol");
var FranklinContract = artifacts.require("./FranklinProxy.sol");
var Depositor = artifacts.require("./PlasmaDepositor.sol");
var Exitor = artifacts.require("./PlasmaExitor.sol");
var Transactor = artifacts.require("./PlasmaTransactor.sol");

module.exports = async function(deployer) {
    let m = await deployer.deploy(Migrations);

    await deployer.deploy(Exitor);
    let ex = await Exitor.deployed();

    await deployer.deploy(Transactor);
    let tr = await Transactor.deployed();

    await deployer.deploy(Depositor);
    let dep = await Depositor.deployed();

    let paddingPubKey = JSON.parse(process.env.PADDING_PUB_KEY);
    // console.log("FranklinContract size = " + (FranklinContract.bytecode.length - 2)/2 );
    await deployer.deploy(FranklinContract, dep.address, tr.address, ex.address);
    let franklin = await FranklinContract.deployed();

    await franklin.deposit(paddingPubKey, 0);
};
