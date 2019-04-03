"use strict";

var OS = require("os")
  , path = require('path').dirname(require.main.filename)
  , exec = require('child_process').exec
  , puts = function(error, stdout, stderr) { if (error) console.log("Error: " + error);};

// Regenerate config if not a windows platform
// if windows, use the default config.h
if (OS.platform() !== "win32") {
  exec("make clean", {"cwd": path + "/scrypt/scrypt-1.2.0"});
  exec("./configure", {cwd: path + "/scrypt/scrypt-1.2.0"}, puts);
}
