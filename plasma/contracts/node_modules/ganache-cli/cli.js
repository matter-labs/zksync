#!/usr/bin/env node

// make sourcemaps work!
require("source-map-support/register")

var yargs = require("yargs");
var pkg = require("./package.json");
var ganache;
try {
  ganache = require("./lib");
} catch(e) {
  ganache = require("./build/ganache-core.node.cli.js");
}
var to = ganache.to;
var URL = require("url");
var fs = require("fs");
var initArgs = require("./args")
var BN = require("bn.js");

var detailedVersion = "Ganache CLI v" + pkg.version + " (ganache-core: " + ganache.version + ")";

var isDocker = "DOCKER" in process.env && process.env.DOCKER.toLowerCase() === "true";
var argv = initArgs(yargs, detailedVersion, isDocker).argv;

function parseAccounts(accounts) {
  function splitAccount(account) {
    account = account.split(',')
    return {
      secretKey: account[0],
      balance: account[1]
    };
  }

  if (typeof accounts === 'string')
    return [ splitAccount(accounts) ];
  else if (!Array.isArray(accounts))
    return;

  var ret = []
  for (var i = 0; i < accounts.length; i++) {
    ret.push(splitAccount(accounts[i]));
  }
  return ret;
}

if (argv.d) {
  argv.s = "TestRPC is awesome!"; // Seed phrase; don't change to Ganache, maintain original determinism
}

if (typeof argv.unlock == "string") {
  argv.unlock = [argv.unlock];
}

var logger = console;

// If quiet argument passed, no output
if (argv.q === true){
  logger = {
    log: function() {}
  };
}

// If the mem argument is passed, only show memory output,
// not transaction history.
if (argv.mem === true) {
  logger = {
    log: function() {}
  };

  setInterval(function() {
    console.log(process.memoryUsage());
  }, 1000);
}

var options = {
  port: argv.p,
  hostname: argv.h,
  debug: argv.debug,
  seed: argv.s,
  mnemonic: argv.m,
  total_accounts: argv.a,
  default_balance_ether: argv.e,
  blockTime: argv.b,
  gasPrice: argv.g,
  gasLimit: argv.l,
  accounts: parseAccounts(argv.account),
  unlocked_accounts: argv.unlock,
  fork: argv.f,
  network_id: argv.i,
  verbose: argv.v,
  secure: argv.n,
  db_path: argv.db,
  account_keys_path: argv.acctKeys,
  vmErrorsOnRPCResponse: !argv.noVMErrorsOnRPCResponse,
  logger: logger,
  allowUnlimitedContractSize: argv.allowUnlimitedContractSize,
  time: argv.t,
  keepAliveTimeout: argv.keepAliveTimeout
}

var fork_address;

// If we're forking from another client, don't try to use the same port.
if (options.fork) {
  var split = options.fork.split("@");
  fork_address = split[0];
  var block;
  if (split.length > 1) {
    block = split[1];
  }

  if (URL.parse(fork_address).port == options.port) {
    options.port = (parseInt(options.port) + 1);
  }

  options.fork = fork_address + (block != null ? "@" + block : "");
}

var server = ganache.server(options);

console.log(detailedVersion);

server.listen(options.port, options.hostname, function(err, result) {
  if (err) {
    console.log(err);
    return;
  }

  var state = result ? result : server.provider.manager.state;

  console.log("");
  console.log("Available Accounts");
  console.log("==================");

  var accounts = state.accounts;
  var addresses = Object.keys(accounts);

  addresses.forEach(function(address, index) {
    var balance = new BN(accounts[address].account.balance).divRound(new BN("1000000000000000000")).toString();
    var line = "(" + index + ") " + address + " (~" + balance + " ETH)";

    if (state.isUnlocked(address) == false) {
      line += " 🔒";
    }

    console.log(line);
  });

  console.log("");
  console.log("Private Keys");
  console.log("==================");

  addresses.forEach(function(address, index) {
    console.log("(" + index + ") " + "0x" + accounts[address].secretKey.toString("hex"));
  });


  if (options.account_keys_path != null) {
    console.log("");
    console.log("Saving accounts and keys to " + options.account_keys_path);
    var obj = {}
    obj.addresses = accounts;
    obj.private_keys = {};
    addresses.forEach(function(address, index) {
       obj.private_keys[address] = accounts[address].secretKey.toString("hex");
    });
    var json = JSON.stringify(obj);
    fs.writeFile(options.account_keys_path, json, 'utf8',function(err){
      if(err) throw err;
    })
  }

  if (options.accounts == null) {
    console.log("");
    console.log("HD Wallet");
    console.log("==================");
    console.log("Mnemonic:      " + state.mnemonic);
    console.log("Base HD Path:  " + state.wallet_hdpath + "{account_index}")
  }

  if (options.gasPrice) {
    console.log("");
    console.log("Gas Price");
    console.log("==================");
    console.log(options.gasPrice);
  }

  if (options.gasLimit) {
    console.log("");
    console.log("Gas Limit");
    console.log("==================");
    console.log(options.gasLimit);
  }

  if (options.fork) {
    console.log("");
    console.log("Forked Chain");
    console.log("==================");
    console.log("Location:    " + fork_address);
    console.log("Block:       " + to.number(state.blockchain.fork_block_number));
    console.log("Network ID:  " + state.net_version);
    console.log("Time:        " + (state.blockchain.startTime || new Date()).toString());
  }

  console.log("");
  console.log("Listening on " + options.hostname + ":" + options.port);
});

process.on('uncaughtException', function(e) {
  console.log(e.stack);
  process.exit(1);
})

// See http://stackoverflow.com/questions/10021373/what-is-the-windows-equivalent-of-process-onsigint-in-node-js
if (process.platform === "win32") {
  require("readline").createInterface({
    input: process.stdin,
    output: process.stdout
  })
  .on("SIGINT", function () {
    process.emit("SIGINT");
  });
}

process.on("SIGINT", function () {
  // graceful shutdown
  server.close(function(err) {
    if (err) {
      console.log(err.stack || err);
    }
    process.exit();
  });
});
