use actix_web::web::Data;
use actix_web::{Error, HttpRequest, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use futures::future::{ready, Ready};
use qstring::QString;
use serde::Serialize;
use serde_json::Value;

use zksync_types::network::Network;

use crate::api_server::rest::v02::error::UnreachableError;
use crate::api_server::rest::v02::{error, ApiVersion, SharedData};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<Value>,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize)]
struct Response {
    request: Request,
    status: ResultStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<error::Error>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
}

pub enum ApiResult<R: Serialize, E: error::ApiError = UnreachableError> {
    Ok(R),
    Error(E),
}

impl<R: Serialize, E: error::ApiError> Responder for ApiResult<R, E> {
    type Error = Error;
    type Future = Ready<Result<HttpResponse, Error>>;

    fn respond_to(self, req: &HttpRequest) -> Self::Future {
        let data = req
            .app_data::<Data<SharedData>>()
            .expect("Wrong app data type");
        let mut args = serde_json::json!({});
        let obj = args.as_object_mut().unwrap();
        for arg in req.match_info().iter() {
            obj.insert(arg.0.to_string(), arg.1.to_string().into());
        }
        let query_string = QString::from(req.query_string());
        for arg in query_string {
            if !obj.contains_key(&arg.0) {
                obj.insert(arg.0, arg.1.into());
            }
        }
        let args = if obj.is_empty() { None } else { Some(args) };

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
                error: Some(err.into()),
            },
        };

        let body = serde_json::to_string(&response).expect("Should be correct serializable");

        ready(Ok(HttpResponse::Ok()
            .content_type("application/json")
            .body(body)))
    }
}

impl<R: Serialize> From<R> for ApiResult<R, UnreachableError> {
    fn from(res: R) -> Self {
        Self::Ok(res)
    }
}
