async function setup() {

    let args = process.argv.slice(2)
    await new Promise(resolve => setTimeout(resolve, args[0]*1000 || 0))

    let Web3 = require('web3')
    let web3 = new Web3(new Web3.providers.HttpProvider("http://localhost:8545"))
    let eth = web3.eth
    let personal = web3.eth.personal

    // might be useful: process.env.PRIVATE_KEY
    let pkeys = [
        '93e1b31cd700c582995dba7bfcca8e9b03effa1e54168f73f618d44e2e730e9c',
        'aa8564af9bef22f581e99125d1829b76c45d08e4f6f0b74d586911f4318b6776',
        'd9ade5186d09f523773611fe31f16f8e7b75ff57d4879dfe38cef5125eeb3885',
        '54a18890db30be68ddc20424c8b20c322f325741d0af1b70b780c424fe973bdf',
        'bc35f5e10eda4e0acdf5dbb2a3f6fe7bedded5526191b28b7faac35074922a1f',
        'f6a401a329ff7b0ac1d09428930677fdabfc5aae5f9bc5e0f8dd863c85ef32f3',
        '22c4b373706e6d748c2abfc1c44dad6ad1cec0b06354259c44668a4cadd63565',
        'f5f17d35eb238908b3ec3462dcf4ad8e8d84ec09c0d1587e3f5feb8a95686baa',
        '5abaea8f281af348587a83c05af384865507d22e111cbc865b1d2be94db84b46',
        'e7b69a24a4154712874791698682acb865884f45cacc1e2100310c23b95fa781',        

        // from run.sh
        '12B7678FF12FE8574AB74FFD23B5B0980B64D84345F9D637C2096CA0EF587806',
    ]

    let prefunded = (await personal.getAccounts())[0]

    for(let i in pkeys) {
        let account = await personal.importRawKey(pkeys[i], '')
        let tx = {from: prefunded, to: account, value: web3.utils.toWei("100", "ether")}
        personal.sendTransaction(tx, '')

        console.log('created and funded account ', account)
    }

}

setup()