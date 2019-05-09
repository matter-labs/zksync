var Migrations = artifacts.require("./Migrations.sol")
var FranklinProxy = artifacts.require("./FranklinProxy.sol")
var Depositor = artifacts.require("./Depositor.sol")
var Exitor = artifacts.require("./Exitor.sol")
var Transactor = artifacts.require("./Transactor.sol")

var ethers = require('ethers')

module.exports = async function(deployer) {
    let m = await deployer.deploy(Migrations)

    await deployer.deploy(Exitor)
    let ex = await Exitor.deployed()

    await deployer.deploy(Transactor)
    let tr = await Transactor.deployed()

    await deployer.deploy(Depositor)
    let dep = await Depositor.deployed()

    let paddingPubKey = JSON.parse(process.env.PADDING_PUB_KEY)
    await deployer.deploy(FranklinProxy, dep.address, tr.address, ex.address)
    let franklin = await FranklinProxy.deployed()
    let value = ethers.utils.parseEther("0.001")
    await franklin.deposit(paddingPubKey, 0, {value})
}
