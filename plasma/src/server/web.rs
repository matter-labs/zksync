use futures::future;

use hyper::rt::{Future, Stream};
use hyper::service::service_fn;
use hyper::{Body, Chunk, Client, Method, Request, Response, Server, StatusCode, header};

/// We need to return different futures depending on the route matched,
/// and we can do that with an enum, such as `futures::Either`, or with
/// trait objects.
///
/// A boxed Future (trait object) is used as it is easier to understand
/// and extend with more types. Advanced users could switch to `Either`.
type BoxFut = Box<Future<Item = Response<Body>, Error = hyper::Error> + Send>;


// // Convert to uppercase before sending back to client.
// (&Method::POST, "/echo/uppercase") => {
//     let mapping = req.into_body().map(|chunk| {
//         chunk
//             .iter()
//             .map(|byte| byte.to_ascii_uppercase())
//             .collect::<Vec<u8>>()
//     });

//     *response.body_mut() = Body::wrap_stream(mapping);
// }

// // Reverse the entire body before sending back to the client.
// //
// // Since we don't know the end yet, we can't simply stream
// // the chunks as they arrive. So, this returns a different
// // future, waiting on concatenating the full body, so that
// // it can be reversed. Only then can we return a `Response`.
// (&Method::POST, "/echo/reversed") => {
//     let reversed = req.into_body().concat2().map(move |chunk| {
//         let body = chunk.iter().rev().cloned().collect::<Vec<u8>>();
//         *response.body_mut() = Body::from(body);
//         response
//     });

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