var tar = require('tar');
var fstream = require('fstream');
var zlib = require('zlib');
var mout = require('mout');
var fs = require('fs');
var bluebird = require('bluebird');

/**
 * Construct and setup all tarball functions
 * @param {Object} [zoptions] The zlib options https://goo.gl/R40wrD
 * @param {Object} [toptions] The tar options https://goo.gl/bqklaR
 * @class
 */
var TarGz = module.exports = function(zoptions, toptions) {

  // Chech if it's a new instance, otherwise initiate a new one
  if (!(this instanceof TarGz))
    return new TarGz(zoptions, toptions);

  // Clone our options to avoid reference errors
  this._options = {
    zlib: zoptions ? mout.lang.deepClone(zoptions) : {},
    tar: toptions ? mout.lang.deepClone(toptions) : {}
  };
};

/**
 * Creates a readstream that transforms a directory into a
 * tarball stream that can be piped
 * @param  {String} directory The directory to be tarballed
 * @return {Stream} A ReadStream that can be piped
 */
TarGz.prototype.createReadStream = function(directory) {

  // Create all needed streams
  var stream1 = fstream.Reader(directory);
  var stream2 = tar.Pack(this._options.tar);
  var stream3 = zlib.createGzip(this._options.zlib);

  // Bubble erros
  this._bubble(stream3, stream2, stream1);

  return stream1.pipe(stream2).pipe(stream3);
};

/**
 * Creates a writestream that receives a tarball stream and decompress
 * to a destination directory
 * @param  {String} directory The directory where files will be extracted
 * @return {Stream} A stream that you can write a tarball file
 */
TarGz.prototype.createWriteStream = function(directory) {
  var stream1 = zlib.createGunzip(this._options.zlib);
  var stream2 = tar.Extract({
    path: directory,
    strip: this._options.tar.strip || 0
  });

  this._bubble(stream1, stream2);
  stream1.pipe(stream2);

  return stream1;
};

/**
 * Parse a tarball stream and emit entry event for each entry parsed inside
 * the piped tarball
 * @return {Stream} A gunzip stream that also emits entry
 */
TarGz.prototype.createParseStream = function() {
  var stream1 = zlib.createGunzip(this._options.zlib);
  var stream2 = tar.Parse();

  this._bubble(stream1, stream2);

  // Capture the entry event
  stream2.on('entry', function(entry) {
    stream1.emit('entry', entry);
  });

  stream1.pipe(stream2);
  return stream1;
};

/**
 * A sugar method to compress a directory to a file
 * @param  {String} source The directory the be tarballed
 * @param  {String} destination The file where the result will be wrote
 * @param  {Function} [cb] An optinal callback that will be called when the
 * job is done
 * @return {Promise} An promise that will be fulfilled when the job is done
 */
TarGz.prototype.compress = bluebird.method(function(source, destination, cb) {
  var def = bluebird.defer();

  // Handle callbacks
  def.promise
    .then(function() {
      if (cb)
        process.nextTick(function() {
          cb();
        });
    })
    .catch(function(err) {
      if (cb)
        process.nextTick(function() {
          cb(err);
        });
    });

  // Create all streams that we need
  var write = fs.createWriteStream(destination);
  var read = this.createReadStream(source);

  // Listen to events
  write.on('error', def.callback);
  write.on('finish', def.callback);
  read.on('error', def.callback);

  // Pipe everything
  read.pipe(write);

  return def.promise;
});

/**
 * A sugar method to decompress into a directory
 * @param  {String} source The tarball to be extracted
 * @param  {String} destination A folder where the tarball will be extracted
 * @param  {Function} [cb] An optinal callback that will be called when the
 * job is done
 * @return {Promise} An promise that will be fulfilled when the job is done
 */
TarGz.prototype.extract = bluebird.method(function(source, destination, cb) {
  var def = bluebird.defer();

  // Handle callbacks
  def.promise
    .then(function() {
      if (cb)
        process.nextTick(function() {
          cb();
        });
    })
    .catch(function(err) {
      if (cb)
        process.nextTick(function() {
          cb(err);
        });
    });

  // Create all streams that we need
  var read = fs.createReadStream(source);
  var write = this.createWriteStream(destination);

  // Listen to events
  write.on('error', def.callback);
  write.on('finish', def.callback);
  read.on('error', def.callback);

  // Pipe everything
  read.pipe(write);

  return def.promise;
});

/**
 * Internal tool for bubbling stream errors
 * @param {Stream} destination The final stream where other streams should have
 * their errors bubbled
 * @param {...Stream} stream Streams that will emit error to the final stream
 */
TarGz.prototype._bubble = function( /* destination, stream, ... */ ) {
  var streams = Array.prototype.slice.call(arguments);
  var destination = streams.shift();

  streams.forEach(function(stream) {
    stream.on('error', function(err) {
      destination.emit('error', err);
    });
  });

};
