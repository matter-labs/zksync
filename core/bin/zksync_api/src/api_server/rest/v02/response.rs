// Built-in uses
use std::collections::HashMap;
use std::convert::From;

// External uses
use actix_web::{web::Data, Error as ActixError, HttpRequest, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use futures::future::{ready, Ready};
use qstring::QString;
use serde::Serialize;
use serde_json::Value;

// Workspace uses
use zksync_types::network::Network;

// Local uses
use super::{error::Error, ApiVersion, SharedData};

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ResultStatus {
    Success,
    Error,
}

#[derive(Serialize)]
struct Request {
    network: Network,
    api_version: ApiVersion,
    resource: String,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    args: HashMap<String, String>,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize)]
struct Response {
    request: Request,
    status: ResultStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Error>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
}

// This struct is needed to wrap all api responses is `Response` struct by implementing `Responder` trait for it.
// We can't use simple `Result`, because `actix-web` has already `Responder` implementation for it.
// Because of this we can't use '?' operator in implementations of endpoints.
pub enum ApiResult<R: Serialize> {
    Ok(R),
    Error(Error),
}

impl<R: Serialize> Responder for ApiResult<R> {
    type Error = ActixError;
    type Future = Ready<Result<HttpResponse, ActixError>>;

    fn respond_to(self, req: &HttpRequest) -> Self::Future {
        let data = req
            .app_data::<Data<SharedData>>()
            .expect("Wrong app data type");
        let mut args = HashMap::new();
        for (name, value) in req.match_info().iter() {
            args.insert(name.to_string(), value.to_string());
        }
        let query_string = QString::from(req.query_string());
        for (name, value) in query_string {
            args.insert(name, value);
        }

        let request = Request {
            network: data.net,
            api_version: data.api_version,
            resource: String::from(req.path()),
            args,
            timestamp: Utc::now(),
        };

        let response = match self {
            ApiResult::Ok(res) => Response {
                request,
                status: ResultStatus::Success,
                result: Some(serde_json::to_value(res).unwrap()),
                error: None,
            },
            ApiResult::Error(err) => Response {
                request,
                status: ResultStatus::Error,
                result: None,
                error: Some(err),
            },
        };

        let body = serde_json::to_string(&response).expect("Should be correct serializable");

        ready(Ok(HttpResponse::Ok()
            .content_type("application/json")
            .body(body)))
    }
}

impl<R: Serialize> From<Error> for ApiResult<R> {
    fn from(err: Error) -> Self {
        Self::Error(err)
    }
}

impl<R: Serialize> From<Result<R, Error>> for ApiResult<R> {
    fn from(result: Result<R, Error>) -> Self {
        match result {
            Ok(ok) => Self::Ok(ok),
            Err(err) => Self::Error(err),
        }
    }
}
