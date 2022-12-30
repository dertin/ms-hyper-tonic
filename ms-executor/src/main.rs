use uuid::Uuid;

// HTTP/1 server - Hyper.rs
use hyper::http::Version;
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
    req: Request<Body>,
    mut grpc_client: HttpClient<tonic::transport::Channel>,
) -> Result<Response<Body>, hyper::Error> {

    let uuid = Uuid::new_v4().to_string();
    let str_method = req.method().to_string();
    let str_uri = req.uri().to_string();
    let str_version = match req.version() {
        Version::HTTP_09 => "HTTP/0.9",
        Version::HTTP_10 => "HTTP/1.0",
        Version::HTTP_11 => "HTTP/1.1",
        Version::HTTP_2 => "HTTP/2.0",
        Version::HTTP_3 => "HTTP/3.0",
        _ => "",
    };
    let mut vec_headers = Vec::new();
    for header in req.headers(){
        vec_headers.push(Header {
            key: header.0.to_string(),
            values: vec![header.1.to_str().unwrap_or_default().to_string()],
        })
    }
    let vec_body = hyper::body::to_bytes(req.into_body()).await.unwrap().to_vec();

    let grpc_request = tonic::Request::new(HttpRequest {
        id: uuid,
        version: str_version.to_string(),
        method: str_method,
        uri: str_uri,
        body: vec_body,
        headers: vec_headers,
    });

    // Send message to grpc server
    let grpc_response: tonic::Response<HttpResponse> = grpc_client.handle(grpc_request).await.unwrap();

    //TODO:
    let res = Response::builder()
        .status(hyper::StatusCode::OK)
        .body(Body::empty())
        .unwrap();

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
