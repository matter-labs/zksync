pub type ABI = (&'static [u8], &'static str);

// pub const TEST_PLASMA_ALWAYS_VERIFY: ABI = (
//     include_bytes!("../../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.abi"),
//     include_str!("../../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.bin"),
// );

pub const TEST_PLASMA2_ALWAYS_VERIFY: &str =
    include_str!("../../../contracts/build/Franklin.json");

// pub const PROD_PLASMA: ABI = (
//     include_bytes!("../../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.abi"),
//     include_str!("../../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.bin"),
// );
