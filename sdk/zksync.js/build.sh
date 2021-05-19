# Removing the old build
rm -rf ./build

# Compiling typescript files
yarn tsc

# Copying typechain information
cp ./src/typechain/*.d.ts ./build/typechain
