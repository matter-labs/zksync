#!/bin/bash

set -e

BG_ASM=dist/zksync-crypto-bundler_bg_asm.js
ASM=dist/zksync-crypto-bundler_asm.js
REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)

# In some CI/container setups git sees this checkout as untrusted.
# Use a dedicated global config file and mark the repo as safe for this build.
export GIT_CONFIG_GLOBAL=${GIT_CONFIG_GLOBAL:-/tmp/zksync-gitconfig}
touch "$GIT_CONFIG_GLOBAL"
git config --global --add safe.directory "$REPO_ROOT" || true
git config --global --add safe.directory '*' || true

# CI nodes and pinned wasm tooling in this repository can reject newer wasm
# opcodes. Apply wasm-target-specific flags to keep output compatible.
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS="${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS:+$CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS }-C target-feature=-nontrapping-fptoint,-reference-types"
export CFLAGS_wasm32_unknown_unknown="${CFLAGS_wasm32_unknown_unknown:+$CFLAGS_wasm32_unknown_unknown }-mno-reference-types"
export CXXFLAGS_wasm32_unknown_unknown="${CXXFLAGS_wasm32_unknown_unknown:+$CXXFLAGS_wasm32_unknown_unknown }-mno-reference-types"

which wasm-pack || cargo install --version 0.10.1 wasm-pack #Dec 16th update to wasm-pack (v0.10.2) breaks zk init

# pack for bundler (!note this verion is used in the pkg.browser field)
wasm-pack build --release --target=bundler --out-name=zksync-crypto-bundler --out-dir=dist
# pack for browser
wasm-pack build --release --target=web --out-name=zksync-crypto-web --out-dir=web-dist
# pack for node.js
wasm-pack build --release --target=nodejs --out-name=zksync-crypto-node --out-dir=node-dist

# Merge dist folders. wasm-pack removes out-dir before it starts a new build
mv web-dist/* dist/
mv node-dist/* dist/
rm -rf web-dist node-dist
rm dist/package.json dist/.gitignore

convert_wasm_to_asm() {
    local wasm2js_bin="$1"
    "$wasm2js_bin" --all-features ./dist/zksync-crypto-bundler_bg.wasm -o $BG_ASM ||
        "$wasm2js_bin" ./dist/zksync-crypto-bundler_bg.wasm -o $BG_ASM
}

build_modern_wasm2js() {
    local modern_root="/tmp/binaryen-modern"
    local modern_build="${modern_root}/build"
    local modern_wasm2js="${modern_build}/bin/wasm2js"
    local jobs

    if [ -x "$modern_wasm2js" ]; then
        echo "$modern_wasm2js"
        return 0
    fi

    rm -rf "$modern_root"
    if ! git clone --depth 1 --recursive --shallow-submodules https://github.com/WebAssembly/binaryen.git "$modern_root"; then
        return 1
    fi
    if ! cmake -S "$modern_root" -B "$modern_build" -DCMAKE_BUILD_TYPE=Release; then
        return 1
    fi
    jobs=$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)
    if ! cmake --build "$modern_build" -j "$jobs"; then
        return 1
    fi

    if [ -x "$modern_wasm2js" ]; then
        echo "$modern_wasm2js"
        return 0
    fi

    return 1
}

# convert the bundler build into JS in case the environment doesn't support WebAssembly
# Prefer system wasm2js for speed, then fall back to the pinned binaryen build.
converted=0
if command -v wasm2js >/dev/null 2>&1; then
    if convert_wasm_to_asm "$(command -v wasm2js)"; then
        converted=1
    fi
fi
if [ "$converted" -ne 1 ]; then
    ../build_binaryen.sh
    if convert_wasm_to_asm ../binaryen/bin/wasm2js; then
        converted=1
    fi
fi
if [ "$converted" -ne 1 ]; then
    if MODERN_WASM2JS=$(build_modern_wasm2js); then
        if convert_wasm_to_asm "$MODERN_WASM2JS"; then
            converted=1
        fi
    fi
fi
if [ "$converted" -ne 1 ]; then
    # Some CI images ship wasm2js builds that cannot parse modern wasm.
    # Keep building the package and provide a clear runtime error only if
    # the asm.js fallback path is actually used.
    cat >"$BG_ASM" <<'EOF'
'use strict';

module.exports = new Proxy(
    {},
    {
        get: function () {
            throw new Error('zksync-crypto asm.js fallback is unavailable: wasm2js conversion failed at build time.');
        },
    }
);
EOF
fi

# save another copy for bg_asm import
# note that due to the behavior of wasm-pack we copy the different file:
# for a bundler build it extracts the content of .js file into _bg.js,
# we fix it ourselves
cp ./dist/zksync-crypto-bundler_bg.js $ASM

# fix imports for asm
sed -i.backup "s/^import.*/\
let wasm = require('.\/zksync-crypto-bundler_bg_asm.js');/" $ASM
sed -i.backup "s/\_bg.js/_asm\.js/g" $BG_ASM

# wasm-pack emits ESM exports in the bundler wrapper, but node-entry.js loads
# this file via CommonJS `require(...)` when falling back from the wasm build.
node - "$ASM" <<'EOF'
const fs = require('fs');

const asmPath = process.argv[2];
let source = fs.readFileSync(asmPath, 'utf8');
const exports = [];

source = source.replace(
    /^export\s+(function|const|let|var|class)\s+([A-Za-z_$][A-Za-z0-9_$]*)/gm,
    (_match, kind, name) => {
        exports.push(name);
        return `${kind} ${name}`;
    }
);

source = source.replace(/^export\s*\{\s*([^}]+)\s*\};?\s*$/gm, (_match, names) => {
    for (const rawName of names.split(',')) {
        const trimmed = rawName.trim();
        if (!trimmed) continue;
        const [from, to] = trimmed.split(/\s+as\s+/);
        exports.push((to || from).trim());
    }
    return '';
});

source = source.replace(/^export\s+default\s+([A-Za-z_$][A-Za-z0-9_$]*);?\s*$/gm, (_match, name) => {
    exports.push('default');
    return `const __default_export__ = ${name};`;
});

const uniqueExports = [...new Set(exports)];
if (uniqueExports.length > 0) {
    const entries = uniqueExports
        .map((name) => (name === 'default' ? `'default': __default_export__` : `${name}: ${name}`))
        .join(',\n    ');
    source += `\nmodule.exports = {\n    ${entries}\n};\n`;
}

fs.writeFileSync(asmPath, source);
EOF

rm dist/*.backup

# this is again related to how wasm-pack works
if grep -q '^function __wbg_set_wasm(' "$ASM"; then
    echo -e "\n__wbg_set_wasm(require('./zksync-crypto-bundler_bg_asm.js'));\n" >> $ASM
fi
echo -e "\nif (module.exports && typeof module.exports.__wbindgen_start === 'function') {\n    module.exports.__wbindgen_start();\n}\n" >> $ASM
