use hyper_util::rt::TokioTimer;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use std::future::Future;
use std::net::SocketAddr;
use std::pin::{pin, Pin};
use std::time::Duration;
use uuid::Uuid;

// HTTP server - Hyper.rs
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::http::Version;
use hyper::service::Service;
use hyper::{Request, Response};

// httpgrpc - protos
use protos::httpgrpc::http_client::HttpClient;
use protos::httpgrpc::{Header, HttpRequest, HttpResponse};

async fn handle_request(
    http_request: Request<Incoming>,
    mut grpc_client: HttpClient<tonic::transport::Channel>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
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

    let http_body: Vec<u8> = http_request
        .into_body()
        .collect()
        .await?
        .to_bytes()
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

    let res_body = http_body_util::Full::from(grpc_response_ref.body);
    let mut res = Response::builder()
        .version(res_version)
        .status(res_status)
        .body(res_body)
        .unwrap();

    let headers_mut = res.headers_mut();
    let res_headers: Vec<Header> = grpc_response_ref.headers.clone();

    for header in res_headers {
        let string_key = header.key.to_owned();
        if let Ok(str_key) = <hyper::header::HeaderName as std::str::FromStr>::from_str(&string_key)
        {
            for string_values in header.values {
                if let Ok(str_value) = hyper::header::HeaderValue::from_str(&string_values) {
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

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("Listening on {}", addr);

    let svc = Svc { grpc_client };

    let mut server =
        hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new());

    server
        .http1()
        .preserve_header_case(true)
        .title_case_headers(true)
        .max_headers(100)
        .timer(TokioTimer::new())
        .header_read_timeout(Duration::from_secs(30))
        .keep_alive(true)
        .header_read_timeout(Duration::from_secs(30));

    server.http2().max_concurrent_streams(200);

    let graceful = hyper_util::server::graceful::GracefulShutdown::new();
    let mut ctrl_c = pin!(tokio::signal::ctrl_c());

    loop {
        tokio::select! {
            conn = listener.accept() => {
                let (stream, peer_addr) = match conn {
                    Ok(conn) => conn,
                    Err(e) => {
                        eprintln!("accept error: {}", e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };
                println!("incomming connection accepted: {}", peer_addr);

                let stream = hyper_util::rt::TokioIo::new(Box::pin(stream));
                let svc_clone = svc.clone();
                let conn = server.serve_connection(stream, svc_clone);

                let conn = graceful.watch(conn.into_owned());

                tokio::spawn(async move {
                    if let Err(err) = conn.await {
                        eprintln!("connection error: {}", err);
                    }
                    println!("connection dropped: {}", peer_addr);
                });
            },
            _ = ctrl_c.as_mut() => {
                drop(listener);
                println!("Ctrl-C received, starting shutdown");
                break;
            }
        }
    }

    tokio::select! {
        _ = graceful.shutdown() => {
            println!("Gracefully shutdown!");
        },
        _ = tokio::time::sleep(Duration::from_secs(10)) => {
            eprintln!("Waited 10 seconds for graceful shutdown, aborting...");
        }
    }
}

#[derive(Debug, Clone)]
struct Svc {
    grpc_client: HttpClient<tonic::transport::Channel>,
}

impl Service<Request<Incoming>> for Svc {
    type Response = Response<http_body_util::Full<Bytes>>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let grpc_client_clone = self.grpc_client.clone();
        Box::pin(async move {
            let result = handle_request(req, grpc_client_clone).await;
            result
        })
    }
}
