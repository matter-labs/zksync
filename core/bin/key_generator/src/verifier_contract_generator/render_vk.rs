//! On detail of how functions in this module works consult @shamatar, in case any changes needed go to https://github.com/matter-labs/solidity_plonk_verifier
//! My own change in this function is addition of key getter name for rendered_key function

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use handlebars::to_json;

use zksync_crypto::bellman::plonk::domains::Domain;
use zksync_crypto::ff::{PrimeField, PrimeFieldRepr};
use zksync_crypto::franklin_crypto::bellman::plonk::better_cs::{
    cs::PlonkCsWidth4WithNextStepParams, keys::VerificationKey,
};
use zksync_crypto::pairing::{CurveAffine, Engine};
use zksync_crypto::{Engine as NodeEngine, Fr};

pub(crate) fn rendered_key(
    key_getter_name: &str,
    verification_key: impl AsRef<Path>,
) -> serde_json::Value {
    let vk = VerificationKey::<NodeEngine, PlonkCsWidth4WithNextStepParams>::read(
        File::open(verification_key).expect("Failed to open verfifcation key file"),
    )
    .expect("Failed to read verification key");
    let mut map = HashMap::new();
    let domain_size = vk.n.next_power_of_two().to_string();
    map.insert("domain_size".to_owned(), to_json(domain_size));
    let num_inputs = vk.num_inputs.to_string();
    map.insert("num_inputs".to_owned(), to_json(num_inputs));
    let domain = Domain::<Fr>::new_for_size(vk.n.next_power_of_two() as u64).unwrap();
    let omega = domain.generator;
    map.insert("omega".to_owned(), to_json(render_scalar_to_hex(&omega)));
    for (i, c) in vk.selector_commitments.iter().enumerate() {
        let rendered = render_g1_affine_to_hex::<NodeEngine>(&c);

        for (j, rendered) in rendered.iter().enumerate() {
            map.insert(
                format!("selector_commitment_{}_{}", i, j),
                to_json(rendered),
            );
        }
    }
    for (i, c) in vk.next_step_selector_commitments.iter().enumerate() {
        let rendered = render_g1_affine_to_hex::<NodeEngine>(&c);

        for (j, rendered) in rendered.iter().enumerate() {
            map.insert(
                format!("next_step_selector_commitment_{}_{}", i, j),
                to_json(rendered),
            );
        }
    }
    for (i, c) in vk.permutation_commitments.iter().enumerate() {
        let rendered = render_g1_affine_to_hex::<NodeEngine>(&c);
        for (j, rendered) in rendered.iter().enumerate() {
            map.insert(
                format!("permutation_commitment_{}_{}", i, j),
                to_json(rendered),
            );
        }
    }
    for (i, c) in vk.non_residues.into_iter().enumerate() {
        let rendered = render_scalar_to_hex(&c);
        map.insert(format!("permutation_non_residue_{}", i), to_json(&rendered));
    }
    let rendered = render_g2_affine_to_hex(&vk.g2_elements[1]);
    map.insert("g2_x_x_c0".to_owned(), to_json(&rendered[0]));
    map.insert("g2_x_x_c1".to_owned(), to_json(&rendered[1]));
    map.insert("g2_x_y_c0".to_owned(), to_json(&rendered[2]));
    map.insert("g2_x_y_c1".to_owned(), to_json(&rendered[3]));

    map.insert("key_getter_name".to_string(), to_json(key_getter_name));
    to_json(map)
}

fn render_scalar_to_hex<F: PrimeField>(el: &F) -> String {
    let mut buff = vec![];
    let repr = el.into_repr();
    repr.write_be(&mut buff).unwrap();

    format!("0x{}", hex::encode(buff))
}

fn render_g1_affine_to_hex<E: Engine>(point: &E::G1Affine) -> [String; 2] {
    if point.is_zero() {
        return ["0x0".to_owned(), "0x0".to_owned()];
    }

    let (x, y) = point.into_xy_unchecked();
    [render_scalar_to_hex(&x), render_scalar_to_hex(&y)]
}

fn render_g2_affine_to_hex(point: &<NodeEngine as Engine>::G2Affine) -> [String; 4] {
    if point.is_zero() {
        return [
            "0x0".to_owned(),
            "0x0".to_owned(),
            "0x0".to_owned(),
            "0x0".to_owned(),
        ];
    }

    let (x, y) = point.into_xy_unchecked();

    [
        render_scalar_to_hex(&x.c0),
        render_scalar_to_hex(&x.c1),
        render_scalar_to_hex(&y.c0),
        render_scalar_to_hex(&y.c1),
    ]
}
