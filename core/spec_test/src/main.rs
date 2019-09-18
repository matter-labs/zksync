use actix_web::{web, App, HttpServer, Responder};
use log::info;
use models::node::tx::{FranklinTx, PackedPublicKey, TxSignature};
use models::node::AccountAddress;
use models::node::FullExit;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct PubkeyPoint {
    pub_key: PackedPublicKey,
}

#[derive(Serialize)]
struct ResultAddress {
    address: AccountAddress,
}

fn address(req: web::Json<PubkeyPoint>) -> impl Responder {
    info!("PubKey: {:?}", (req.0.pub_key.0).0.into_xy());
    let address = AccountAddress::from_pubkey((req.0).pub_key.0);
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
    valid: bool,
}
fn check_tx_signature(req: web::Json<FranklinTx>) -> impl Responder {
    let tx = req.0;
    info!("tx: {:#?}", tx);
    info!("tx bytes: {}", hex::encode(tx.get_bytes()));
    let valid = tx.check_signature();
    info!("tx signature valid: {}", valid);
    web::Json(TxSignatureResp { valid })
}

#[derive(Serialize)]
struct CheckFullExitResponse {
    valid: bool,
    account_address: Option<AccountAddress>,
}
fn check_full_exit_signature(req: web::Json<FullExit>) -> impl Responder {
    let full_exit = req.0;
    info!("full_exit: {:#?}", full_exit);
    let account_address = full_exit.verify_signature();
    info!("signature valid: {:#?}", account_address.is_some());
    if let Some(account) = &account_address {
        info!("author account: {}", account.to_hex());
    }
    web::Json(CheckFullExitResponse {
        valid: account_address.is_some(),
        account_address,
    })
}

fn main() {
    env_logger::init();

    HttpServer::new(|| {
        App::new()
            .service(web::resource("/address").route(web::post().to(address)))
            .service(web::resource("/check_signature").route(web::post().to(check_signature)))
            .service(web::resource("/check_tx_signature").route(web::post().to(check_tx_signature)))
            .service(
                web::resource("/check_full_exit_signature")
                    .route(web::post().to(check_full_exit_signature)),
            )
    })
    .bind("127.0.0.1:8734")
    .expect("Can not bind to port 8734")
    .run()
    .unwrap();
}
