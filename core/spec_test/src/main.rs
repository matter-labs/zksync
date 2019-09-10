use actix_web::{web, App, HttpRequest, HttpServer, Responder, HttpResponse};
use actix_web::get;

#[get("/hello")]
fn index3() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}



fn main() {
    HttpServer::new(|| {
        App::new()
            .service(index3)
    })
        .bind("127.0.0.1:8734")
        .expect("Can not bind to port 8734")
        .run()
        .unwrap();
}
