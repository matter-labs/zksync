{
  'variables': {
    'compiler-flags': [],
    'scrypt_platform_specific_files': [],
    'scrypt_platform_specific_includes': [],
    'conditions': [
      ['OS=="win"', {
        'scrypt_platform_specific_files': [
          'scrypt/win/mman.c',
          'scrypt/win/gettimeofday.c'
        ],
        'scrypt_platform_specific_includes': [
          'scrypt/win/include',
        ],
      }],
      ['OS!="win"',{
        'compiler-flags': ['-std=c++11']
      }],
      ['OS=="mac"', {
        'xcode_settings': {
          'OTHER_CPLUSPLUSFLAGS' : ['-stdlib=libc++'],
        },
      }],
    ],
  },

  'target_defaults': {
    'default_configuration': 'Release',
    'msvs_settings': {
      'VCCLCompilerTool': {
        'RuntimeLibrary': 0, # static release
        'ExceptionHandling': '2',
        'AdditionalOptions': ['/EHsc'],
        'DisableSpecificWarnings': ['4506'],
      },
    },
  },

  'targets': [
    {
      'target_name': 'copied_files',
      'conditions': [
        ['OS=="win"', {
          'copies' : [{
            'destination':'scrypt/scrypt-1.2.0/',
            'files' : [
              'scrypt/win/include/config.h'
            ]
          }],
        }]
      ]
    },
    {
      'target_name': 'scrypt_lib',
      'type' : 'static_library',
      'sources': [
        'scrypt/scrypt-1.2.0/lib/crypto/crypto_scrypt.c',
        'scrypt/scrypt-1.2.0/lib/crypto/crypto_scrypt_smix.c',
        'scrypt/scrypt-1.2.0/libcperciva/util/warnp.c',
        'scrypt/scrypt-1.2.0/libcperciva/alg/sha256.c',
        'scrypt/scrypt-1.2.0/libcperciva/util/insecure_memzero.c',
        'scrypt/scrypt-1.2.0/lib/scryptenc/scryptenc_cpuperf.c',
        '<@(scrypt_platform_specific_files)',
      ],
      'include_dirs': [
        'scrypt/scrypt-1.2.0/',
        'scrypt/scrypt-1.2.0/libcperciva/cpusupport',
        'scrypt/scrypt-1.2.0/libcperciva/alg',
        'scrypt/scrypt-1.2.0/libcperciva/util',
        'scrypt/scrypt-1.2.0/lib/crypto',
        '<@(scrypt_platform_specific_includes)',
      ],
      'defines': [
        'HAVE_CONFIG_H'
      ],
      'conditions': [
        ['OS=="win"', { 'defines' : [ 'inline=__inline' ] }],
      ],
      'dependencies': ['copied_files'],
    },
    {
      'target_name': 'scrypt_wrapper',
      'type' : 'static_library',
      'sources': [
        'src/util/memlimit.c',
        'src/scryptwrapper/keyderivation.c',
        'src/scryptwrapper/pickparams.c',
        'src/scryptwrapper/hash.c'
      ],
      'include_dirs': [
        'src/scryptwrapper/inc',
        'src',
        'scrypt/scrypt-1.2.0/libcperciva/alg',
        'scrypt/scrypt-1.2.0/libcperciva/util',
        'scrypt/scrypt-1.2.0/lib/crypto',
        'scrypt/scrypt-1.2.0/lib/util/',
        'scrypt/scrypt-1.2.0/lib/scryptenc/',
        '<@(scrypt_platform_specific_includes)',
      ],
      'defines': [
        'HAVE_CONFIG_H'
      ],
      'conditions': [
        ['OS=="win"', { 'defines' : [ 'inline=__inline' ] }],
      ],
    },
    {
      'target_name': 'scrypt',
      'sources': [
        'src/node-boilerplate/scrypt_common.cc',
        'src/node-boilerplate/scrypt_params_async.cc',
        'src/node-boilerplate/scrypt_params_sync.cc',
        'src/node-boilerplate/scrypt_kdf_async.cc',
        'src/node-boilerplate/scrypt_kdf_sync.cc',
        'src/node-boilerplate/scrypt_kdf-verify_sync.cc',
        'src/node-boilerplate/scrypt_kdf-verify_async.cc',
        'src/node-boilerplate/scrypt_hash_sync.cc',
        'src/node-boilerplate/scrypt_hash_async.cc',
        'scrypt_node.cc'
      ],
      'include_dirs': [
        '<!(node -e "require(\'nan\')")',
        'src/util',
        'src/scryptwrapper/inc',
        'src/node-boilerplate/inc'
      ],
      'cflags': ['<@(compiler-flags)'],
      'dependencies': ['scrypt_wrapper','scrypt_lib'],
    }
  ],
}
