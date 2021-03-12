use actix_web::{
    dev::{Body, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpResponse,
};
use async_trait::async_trait;
use chrono::prelude::Utc;
use futures::future::{ok, Ready};
use futures::Future;
use futures::TryStreamExt;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::pin::Pin;
use std::task::{Context, Poll};

#[async_trait(?Send)]
pub trait ParseResponse {
    async fn parse<T: DeserializeOwned>(&mut self) -> Result<T, Box<dyn std::error::Error>>;
}

// TODO: Try to make response middleware without double convertation (ZKS-560)
#[async_trait(?Send)]
impl ParseResponse for ServiceResponse<Body> {
    async fn parse<T>(&mut self) -> Result<T, Box<dyn std::error::Error>>
    where
        T: DeserializeOwned,
    {
        let bytes = self
            .take_body()
            .try_fold(Vec::new(), |mut acc, chunk| async {
                acc.extend(chunk);
                Ok(acc)
            });
        Ok(serde_json::from_slice(&bytes.await?)?)
    }
}

pub struct ResponseTransform;

impl<S> Transform<S> for ResponseTransform
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<Body>, Error = Error>,
    S::Future: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<Body>;
    type Error = Error;
    type InitError = ();
    type Transform = ResponseMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(ResponseMiddleware { service })
    }
}

pub struct ResponseMiddleware<S> {
    service: S,
}

impl<S> Service for ResponseMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<Body>, Error = Error>,
    S::Future: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<Body>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        println!("Hi from start. You requested: {}", req.path());

        let fut = self.service.call(req);
        Box::pin(async move {
            let mut raw_response = fut.await?;
            let result: serde_json::Value = raw_response.parse().await.unwrap();
            
            let response = json!(
            {
                "request": {
                    "network": "localhost", //TODO
                    "api_version": "v0.2",
                    "resource": raw_response.request().path(),
                    "args": {
                        //TODO
                    },
                    "timestamp": Utc::now(),
                },
                "status": "success", // TODO
                "result": result
            });
            let new_res = ServiceResponse::new(
                raw_response.request().clone(),
                HttpResponse::Ok().json(response).into_body(),
            );

            Ok(new_res)
        })
    }
}
