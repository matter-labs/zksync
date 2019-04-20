const sleep = require('sleep')

class Client {

    constructor(id) {
        this.id = id
        console.log(`creating client #${this.id}`)
    }

    send() {

    }
}

var args = process.argv.slice(2)
let nClients = args[0] || 2
let tps = args[1] || 100

console.log(`Usage: npm test -- [nClients] [TPS]`)
console.log(`Starting loadtest for ${nClients} with ${tps} TPS`)

let clients = []

for (let i=0; i<nClients; i++) {
    clients.push(new Client(i))
}

while(true) {
    var nextTick = new Date(new Date().getTime() + 1000);
    for (let i=0; i<tps; i++) {
        process.stdout.write('.')
    }
    console.log('')
    while(nextTick > new Date()) sleep.msleep(1)
}
