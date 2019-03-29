# Change Log
All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/).

## [6.0.2] - 2016-04-17
### Fixed
- Microsoft compile issues

## [5.4.1] - 2015-10-12
### Fixed
- Corrected Hash API documentation in README

## [5.4.0] - 2015-10-09
### Fixed
- Check for empty buffer (see #97)

## [5.3.0] - 2015-10-08
### Added
- This changelog file

### Changed
- Renamed Readme.md to README.md
- Inserted link to changelog in README.md

## [5.2.0] - 2015-10-06
### Fixed
- Allow building on MS 2015

## [5.1.1] - 2015-09-21
### Fixed
- Remove hardcoded nan paths - issue 92

## [5.1.0] - 2015-09-21
### Changed
- Updated Readme documentation to include .....

## [5.0] - 2015-09-13
### Added
- Made module ES6 Promise compatible
- ...

### Fixed
- Fixes ...

### Changed
- C++ addon code rewritten using Nan 2.x
- API has changed:
- Every output is a buffer.
- Separated functions into async and sync versions.
- Api name swap: What was kdf in previous versions is now hash (and vice versa).
- Async functions will return a Promise if no callback function is present and Promises are available (else it will throw a SyntaxError).
- Using correct JavaScript Error object for all errors
- Updated Readme documentation to include .....
