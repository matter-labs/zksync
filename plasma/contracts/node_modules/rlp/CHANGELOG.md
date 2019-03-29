# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/) 
(modification: no type change headlines) and this project adheres to 
[Semantic Versioning](http://semver.org/spec/v2.0.0.html).


## [v2.1.0] - 2018-06-28
- Updated supported Node versions, PR [#13](https://github.com/ethereumjs/rlp/pull/13)
- Switched to ``safe-buffer`` for backwards compatibility, PR [#18](https://github.com/ethereumjs/rlp/pull/18)
- Increased test coverage, PR [#22](https://github.com/ethereumjs/rlp/pull/22)
- Example code tweaks, PR [#12](https://github.com/ethereumjs/rlp/pull/12)
- Fix test runs on Windows, Issue [#7](https://github.com/ethereumjs/rlp/issues/7)
- Added code coverage, PR [#8](https://github.com/ethereumjs/rlp/pull/8)

[v2.1.0]: https://github.com/ethereumjs/rlp/compare/2.0.0...v2.1.0

## [2.0.0] - 2015-09-23
- User ``Buffer`` values as input for encoding

[2.0.0]: https://github.com/ethereumjs/rlp/compare/1.1.2...2.0.0

## [1.1.2] - 2015-09-22
- Fix zero encoding

[1.1.2]: https://github.com/ethereumjs/rlp/compare/1.1.1...1.1.2

## [1.1.1] - 2015-09-21
- Fixes for ``bin``

[1.1.1]: https://github.com/ethereumjs/rlp/compare/1.1.0...1.1.1

## [1.1.0] - 2015-09-21
- Added ``getLength()`` method
- Added hex prefix stripping (``isHexPrefix()`` / ``stripHexPrefix()``)
- Code formatting clean-ups

[1.1.0]: https://github.com/ethereumjs/rlp/compare/1.0.1...1.1.0

## [1.0.1] - 2015-06-27
- Code formatting clean-ups

[1.0.1]: https://github.com/ethereumjs/rlp/compare/1.0.0...1.0.1

## [1.0.0] - 2015-06-06
- Added check for invalid 0
- Hardened rlp

[1.0.0]: https://github.com/ethereumjs/rlp/compare/0.0.14...1.0.0

## Older releases:

- [0.0.14](https://github.com/ethereumjs/rlp/compare/0.0.13...0.0.14) - 2015-03-31
- [0.0.13](https://github.com/ethereumjs/rlp/compare/0.0.12...0.0.13) - 2015-03-30
- [0.0.12](https://github.com/ethereumjs/rlp/compare/0.0.11...0.0.12) - 2014-12-26