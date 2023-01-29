#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use uuid::Uuid;

// HTTP server - Hyper.rs
use hyper::http::{HeaderValue, Version};
use hyper::service::Service;
use hyper::{Body, Request, Response, Server};
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

// httpgrpc - protos
use protos::httpgrpc::http_client::HttpClient;
use protos::httpgrpc::{Header, HttpRequest, HttpResponse};

async fn shutdown_signal() {
    // Wait for the CTRL+C signal
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
}

async fn handle_request(
    http_request: Request<Body>,
    mut grpc_client: HttpClient<tonic::transport::Channel>,
) -> Result<Response<Body>, hyper::Error> {
    // Create grpc request from http request
    let http_uuid = Uuid::new_v4().to_string();
    let http_method = http_request.method().to_string();
    let http_uri = http_request.uri().to_string();
    let http_version = match http_request.version() {
        Version::HTTP_09 => "HTTP/0.9",
        Version::HTTP_10 => "HTTP/1.0",
        Version::HTTP_11 => "HTTP/1.1",
        Version::HTTP_2 => "HTTP/2.0",
        Version::HTTP_3 => "HTTP/3.0",
        _ => "HTTP/1.1",
    };
    let mut http_headers = Vec::new();
    for header in http_request.headers() {
        http_headers.push(Header {
            key: header.0.to_string(),
            values: vec![header.1.to_str().unwrap_or_default().to_string()],
        })
    }
    let http_body: Vec<u8> = hyper::body::to_bytes(http_request.into_body())
        .await
        .unwrap()
        .to_vec();

    let grpc_request: tonic::Request<HttpRequest> = tonic::Request::new(HttpRequest {
        id: http_uuid,
        version: http_version.to_string(),
        method: http_method,
        uri: http_uri,
        body: http_body,
        headers: http_headers,
    });

    // Send request to grpc server
    let grpc_response: tonic::Response<HttpResponse> =
        grpc_client.handle(grpc_request).await.unwrap();

    let grpc_response_ref = grpc_response.get_ref().to_owned();

    // Generate http response from grpc response
    let res_status = grpc_response_ref.status.try_into().unwrap_or(500);
    let res_version = match grpc_response_ref.version.as_str() {
        "HTTP_09" => Version::HTTP_09,
        "HTTP_10" => Version::HTTP_10,
        "HTTP_11" => Version::HTTP_11,
        "HTTP_2" => Version::HTTP_2,
        "HTTP_3" => Version::HTTP_3,
        _ => Version::HTTP_11,
    };

    let res_body = Body::from(grpc_response_ref.body);
    let mut res = Response::builder()
        .version(res_version)
        .status(res_status)
        .body(res_body)
        .unwrap();

    let headers_mut = res.headers_mut();
    let res_headers: Vec<Header> = grpc_response_ref.headers.clone();

    for header in res_headers {
        let string_key = header.key.to_owned();
        if let Ok(str_key) = <hyper::header::HeaderName as std::str::FromStr>::from_str(&string_key) {
            for string_values in header.values {
                if let Ok(str_value) = hyper::header::HeaderValue::from_str(&string_values){
                    headers_mut.insert(&str_key, str_value);
                }
            }
        }
    }

    Ok(res)
}

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    let endpoints = ["http://[::1]:50051"]
        .iter()
        .map(|a| tonic::transport::Channel::from_static(a));
    let channel_tonic = tonic::transport::Channel::balance_list(endpoints);
    let grpc_client = HttpClient::new(channel_tonic);

    let server = Server::bind(&addr)
        .http1_preserve_header_case(true)
        .http1_title_case_headers(true)
        .serve(MakeSvc { grpc_client });

    // And now add a graceful shutdown signal...
    let graceful = server.with_graceful_shutdown(shutdown_signal());

    // Run this server for... forever!
    if let Err(e) = graceful.await {
        eprintln!("server error: {}", e);
    }
}

struct Svc {
    grpc_client: HttpClient<tonic::transport::Channel>,
}

impl Service<Request<Body>> for Svc {
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        Box::pin({
            let grpc_client_clone = self.grpc_client.clone();
            handle_request(req, grpc_client_clone)
        })
    }
}

struct MakeSvc {
    grpc_client: HttpClient<tonic::transport::Channel>,
}

impl<T> Service<T> for MakeSvc {
    type Response = Svc;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: T) -> Self::Future {
        let grpc_client_clone = self.grpc_client.clone();
        let fut = async move {
            Ok(Svc {
                grpc_client: grpc_client_clone,
            })
        };
        Box::pin(fut)
    }
}
