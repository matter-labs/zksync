use std::sync::Arc;
use futures::future;
use futures::future::lazy;

use tokio;
use tokio::runtime::Runtime;

use hyper::rt::{Future, Stream};
use hyper::service::service_fn;
use hyper::{Body, Chunk, Client, Method, Request, Response, Server, StatusCode, header};

use super::account_manager::{APICall, AccountManager};

pub struct APIServer {
    handler: Arc<AccountManager>
}

type BoxFut = Box<Future<Item = Response<Body>, Error = hyper::Error> + Send>;

impl APIServer {

    /// Create web server and start listening in a separate thread with thread-pool for incoming connections
    pub fn new(handler: &Arc<AccountManager>) -> Self {
        let this = Self{
            handler: Arc::clone(handler)
        };

        let handler = Arc::clone(&this.handler);

        let server = future::lazy(move || {
            let addr = ([127, 0, 0, 1], 3000).into();
            println!("Listening on http://{}", addr);

            let new_service = move || {
                let handler = Arc::clone(&handler);
                service_fn(move |req| {
                    let handler = Arc::clone(&handler);
                    Self::router(req, handler)
                })
            };

            Server::bind(&addr)
                .serve(new_service)
                .map_err(|e| eprintln!("server error: {}", e))
        });

        hyper::rt::spawn(server);

        this
    }

    fn listen(&mut self) {
 
    }

    fn router(req: Request<Body>, handler: Arc<AccountManager>) -> BoxFut {
        let mut response = Response::builder()
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::empty())
                        .unwrap();

        println!("new req {:?}", &req);
        match (req.method(), req.uri().path()) {

            (&Method::GET, "/phony") => {
                let data = vec!["foo", "bar"];

                handler.handle(APICall::Phony);

                let json = serde_json::to_string(&data).unwrap();
                *response.body_mut() = Body::from(json);
            }

            (&Method::POST, "/register") => {

                let body = req.into_body();
                println!("post {:?}", body);

                return Box::new( body.concat2().and_then(move |body| {

                    let params: APICall = serde_json::from_slice(&body).unwrap();
                    println!("Params: {:?}", params);

                    handler.handle(params);

                    let data = vec!["ok"];
                    let json = serde_json::to_string(&data).unwrap();
                    *response.body_mut() = Body::from(json);

                    future::ok(response)
                }))
            }

            _ => {
                *response.status_mut() = StatusCode::NOT_FOUND;
                *response.body_mut() = Body::from(r#"route not found"#);
            }
        }

        Box::new(future::ok(response))
    }

}

// /// We need to return different futures depending on the route matched,
// /// and we can do that with an enum, such as `futures::Either`, or with
// /// trait objects.
// ///
// /// A boxed Future (trait object) is used as it is easier to understand
// /// and extend with more types. Advanced users could switch to `Either`.
// type BoxFut = Box<Future<Item = Response<Body>, Error = hyper::Error> + Send>;

// #[derive(Serialize, Deserialize, Debug)]
// struct Params {
//     username: String,
//     password: String
// }

// fn router(req: Request<Body>) -> BoxFut {
//     let mut response = Response::builder()
//                         .header(header::CONTENT_TYPE, "application/json")
//                         .body(Body::empty())
//                         .unwrap();

//     match (req.method(), req.uri().path()) {

//         (&Method::GET, "/json") => {
//             let data = vec!["foo", "bar"];
//             let json = serde_json::to_string(&data).unwrap();
//             *response.body_mut() = Body::from(json);
//         }

//         (&Method::POST, "/json") => {

//             let body = req.into_body();
//             println!("post {:?}", body);

//             return Box::new( body.concat2().and_then(|body| {
//                 let params: Params = serde_json::from_slice(&body).unwrap();
//                 println!("Params: {:?}", params);

//                 let data = vec!["ok!"];
//                 let json = serde_json::to_string(&data).unwrap();
//                 *response.body_mut() = Body::from(json);

//                 future::ok(response)
//             }))
//         }

//         _ => {
//             *response.status_mut() = StatusCode::NOT_FOUND;
//             *response.body_mut() = Body::from(r#"route not found"#);
//         }
//     }

//     Box::new(future::ok(response))
// }

#[test]
fn test_web_srv() {

    //let rt = Runtime::new().unwrap();
        
    // tokio::run(lazy( || {
    //     let man = AccountManager::new();
    //     std::thread::sleep(std::time::Duration::from_secs(9999));
    //     future::ok(())
    // }));

    // let addr = ([127, 0, 0, 1], 3000).into();

    // let server = Server::bind(&addr)
    //     .serve(|| service_fn(router))
    //     .map_err(|e| eprintln!("server error: {}", e));

    // println!("Listening on http://{}", addr);
    // hyper::rt::run(server);


}