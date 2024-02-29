use async_trait::async_trait;
use atrium_api::xrpc::{HttpClient, XrpcClient};
use js_sys::Uint8Array;
use wasm_bindgen::JsValue;
use worker::{Fetch, Headers, Method, RequestInit};

pub(crate) struct FetchClient;

#[async_trait(?Send)]
impl HttpClient for FetchClient {
    async fn send_http(
        &self,
        request: http::Request<Vec<u8>>,
    ) -> Result<http::Response<Vec<u8>>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let uri = request.uri().to_string();
        let init = RequestInit {
            body: if request.body().is_empty() {
                None
            } else {
                let u8array = Uint8Array::new_with_length(request.body().len() as u32);
                u8array.copy_from(request.body());
                Some(JsValue::from(u8array))
            },
            headers: Headers::from(request.headers().clone()),
            method: Method::from(request.method().to_string()),
            ..Default::default()
        };
        let mut response =
            Fetch::Request(worker::Request::new_with_init(&uri, &init).map_err(|e| e.to_string())?)
                .send()
                .await
                .map_err(|e| e.to_string())?;
        let mut builder = http::Response::builder().status(response.status_code());
        for (k, v) in response.headers() {
            builder = builder.header(k, v);
        }
        Ok(builder
            .body(response.bytes().await.map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?)
    }
}

impl XrpcClient for FetchClient {
    fn base_uri(&self) -> String {
        "https://bsky.social".into()
    }
}