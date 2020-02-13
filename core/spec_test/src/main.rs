use actix_web::{web, App, HttpServer, Responder};
use log::info;
use models::node::tx::{FranklinTx, PackedPublicKey, TxSignature};
use models::node::PubKeyHash;
use serde::{Deserialize, Serialize};

use crypto_exports::franklin_crypto;
use crypto_exports::rand;

#[derive(Deserialize)]
struct PubkeyPoint {
    pub_key: PackedPublicKey,
}

#[derive(Serialize)]
struct ResultAddress {
    address: PubKeyHash,
}

fn address(req: web::Json<PubkeyPoint>) -> impl Responder {
    info!("PubKey: {:?}", (req.0.pub_key.0).0.into_xy());
    let address = PubKeyHash::from_pubkey(&(req.0).pub_key.0);
    info!("Address: {}", address.to_hex());
    web::Json(ResultAddress { address })
}

#[derive(Deserialize)]
enum SignatureType {
    MusigPedersen,
    MusigSha256,
}

#[derive(Deserialize)]
struct SignedMessage {
    msg: Vec<u8>,
    signature: TxSignature,
    variant: SignatureType,
}

#[derive(Serialize)]
struct SignedMessageKey {
    correct: bool,
    pk: Option<PackedPublicKey>,
}

fn check_signature(req: web::Json<SignedMessage>) -> impl Responder {
    let signed_msg = req.0;
    let pk = match signed_msg.variant {
        SignatureType::MusigPedersen => signed_msg.signature.verify_musig_pedersen(&signed_msg.msg),
        SignatureType::MusigSha256 => signed_msg.signature.verify_musig_sha256(&signed_msg.msg),
    }
    .map(PackedPublicKey);

    web::Json(SignedMessageKey {
        correct: pk.is_some(),
        pk,
    })
}

#[derive(Serialize)]
struct TxSignatureResp {
    pub_key_hash: Option<PubKeyHash>,
}

fn check_tx_signature(req: web::Json<FranklinTx>) -> impl Responder {
    let tx = req.0;
    info!("tx: {:#?}", tx);
    info!("tx bytes: {}", hex::encode(tx.get_bytes()));
    let pub_key_hash = match tx {
        FranklinTx::Transfer(tx) => tx.verify_signature(),
        FranklinTx::Withdraw(tx) => tx.verify_signature(),
        FranklinTx::Close(tx) => tx.verify_signature(),
        _ => None,
    };
    info!("tx signature pub key hash: {:?}", pub_key_hash);
    web::Json(TxSignatureResp { pub_key_hash })
}

fn main() {
    env_logger::init();

    HttpServer::new(|| {
        App::new()
            .service(web::resource("/address").route(web::post().to(address)))
            .service(web::resource("/check_signature").route(web::post().to(check_signature)))
            .service(web::resource("/check_tx_signature").route(web::post().to(check_tx_signature)))
    })
    .bind("127.0.0.1:8734")
    .expect("Can not bind to port 8734")
    .run()
    .unwrap();
}
