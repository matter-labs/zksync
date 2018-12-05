use std::sync::Arc;
use futures::future;

use hyper::rt::{Future, Stream};
use hyper::service::service_fn;
use hyper::{Body, Chunk, Client, Method, Request, Response, Server, StatusCode, header};

use super::account_manager::{APICall, AccountManager};

pub struct APIServer {
    handler: Arc<AccountManager>
}

impl APIServer {

    /// Create web server and start listening in a separate thread with thread-pool for incoming connections
    pub fn new(handler: &Arc<AccountManager>) -> Self {
        let this = Self{
            handler: Arc::clone(handler)
        };

        //let addr = ([127, 0, 0, 1], 3000).into();

        //let handler = Arc::clone(&self.handler);
        // let server = Server::bind(&addr)
        //     .serve(|| {
        //         let handler = Arc::clone(&self.handler);
        //         service_fn(|req: Request<Body>| {
        //             Self::router(req, handler)
        //         })
        //     })
        //     .map_err(|e| eprintln!("server error: {}", e));

        // println!("Listening on http://{}", addr);
        // hyper::rt::run(server);

        this
    }

    fn listen(&mut self) {
 
    }

    fn router(req: Request<Body>, handler: Arc<AccountManager>) -> BoxFut {
        let mut response = Response::builder()
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::empty())
                        .unwrap();

        match (req.method(), req.uri().path()) {

            (&Method::GET, "/json") => {
                let data = vec!["foo", "bar"];
                let json = serde_json::to_string(&data).unwrap();
                *response.body_mut() = Body::from(json);
            }

            (&Method::POST, "/json") => {

                let body = req.into_body();
                println!("post {:?}", body);

                return Box::new( body.concat2().and_then(|body| {
                    let params: Params = serde_json::from_slice(&body).unwrap();
                    println!("Params: {:?}", params);

                    let data = vec!["foo", "bar"];
                    let json = serde_json::to_string(&data).unwrap();
                    *response.body_mut() = Body::from(json);

                    future::ok(response)
                }))
            }

            _ => {
                *response.status_mut() = StatusCode::NOT_FOUND;
            }
        }

        Box::new(future::ok(response))
    }

}

/// We need to return different futures depending on the route matched,
/// and we can do that with an enum, such as `futures::Either`, or with
/// trait objects.
///
/// A boxed Future (trait object) is used as it is easier to understand
/// and extend with more types. Advanced users could switch to `Either`.
type BoxFut = Box<Future<Item = Response<Body>, Error = hyper::Error> + Send>;

#[derive(Serialize, Deserialize, Debug)]
struct Params {
    username: String,
    password: String
}

fn router(req: Request<Body>) -> BoxFut {
    let mut response = Response::builder()
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::empty())
                        .unwrap();

    match (req.method(), req.uri().path()) {

        (&Method::GET, "/json") => {
            let data = vec!["foo", "bar"];
            let json = serde_json::to_string(&data).unwrap();
            *response.body_mut() = Body::from(json);
        }

        (&Method::POST, "/json") => {

            let body = req.into_body();
            println!("post {:?}", body);

            return Box::new( body.concat2().and_then(|body| {
                let params: Params = serde_json::from_slice(&body).unwrap();
                println!("Params: {:?}", params);

                let data = vec!["foo", "bar"];
                let json = serde_json::to_string(&data).unwrap();
                *response.body_mut() = Body::from(json);

                future::ok(response)
            }))
        }

        _ => {
            *response.status_mut() = StatusCode::NOT_FOUND;
        }
    }

    Box::new(future::ok(response))
}

#[test]
fn test_web_srv() {

    let addr = ([127, 0, 0, 1], 3000).into();

    let server = Server::bind(&addr)
        .serve(|| service_fn(router))
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);
    hyper::rt::run(server);

}