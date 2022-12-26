use uuid::Uuid;
use futures::executor::block_on;

// HTTP/1 server - Hyper.rs
use hyper::server::conn::http1;
use hyper::body::Bytes;
use hyper::{body::Incoming as IncomingBody, Request, Response};
use http_body_util::Full;
use std::net::SocketAddr;
use tokio::net::TcpListener;

// gRPC protos - Tonic
use protos::httpgrpc::http_client::HttpClient;
use protos::httpgrpc::{Header, HttpRequest};
use tonic::transport::Channel;


#[derive(Debug, Clone)]
struct Svc {
    grpc_client: HttpClient<Channel>,
}

async fn util_body_to_vec(req: Request<IncomingBody>) -> Vec<u8> {
    http_body_util::BodyExt::collect(req)
        .await
        .unwrap()
        .to_bytes()
        .into()
}
// Implementacion del servicio HTTP para atender las peticiones entrantes
impl hyper::service::Service<Request<IncomingBody>> for Svc {
    type Response = Response<Full<Bytes>>;
    type Error = hyper::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn call(&mut self, incoming_request: Request<IncomingBody>) -> Self::Future {

        println!(
            "received request! method: {:?}, url: {:?}, headers: {:?}",
            incoming_request.method(),
            incoming_request.uri(),
            incoming_request.headers()
        );

        let uuid_request = Uuid::new_v4().to_string();
        let str_method = incoming_request.method().to_string();
        let str_uri = incoming_request.uri().to_string();
        let str_version = format!("{:?}", incoming_request.version());
        let vec_headers = Header {
            key: "".to_owned(),
            values: vec!["".to_owned()],
        };

        // Await the whole body to be collected into a single `Bytes`...
        let future_body = util_body_to_vec(incoming_request);
        let str_body = block_on(future_body);

        // make gRPC request - http incoming request to grpc outgoing request
        let grpc_request = tonic::Request::new(HttpRequest {
            id: uuid_request,
            version: str_version,
            method: str_method,
            uri: str_uri,
            body: str_body,
            headers: vec![vec_headers],
        });

        println!("Sending request to gRPC Server...");
        
        let future_grpc_response = self.grpc_client.handle(grpc_request);
        let grpc_response = block_on(future_grpc_response).unwrap();
        println!("RESPONSE={:?}", grpc_response);

        Box::pin(async { Ok(Response::new(Full::new(Bytes::from("Hello, World!")))) })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    
    // Listener HTTP Server - Hyper.rs 
    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    let listener = TcpListener::bind(addr).await?;
    
    // New gRPC client - Tonic
    let endpoints = ["http://[::1]:50051", "http://[::1]:50052"]
        .iter()
        .map(|a| Channel::from_static(a));
    let channel = Channel::balance_list(endpoints);
    let grpc_client = HttpClient::new(channel);

    // We start a loop to continuously accept incoming connections
    
        loop {
            let (stream, _) = listener.accept().await?;
            
            let grpc_client_clone = grpc_client.clone();
            println!("{:?}", grpc_client_clone);
            
            // Spawn a tokio task to serve multiple connections concurrently
            tokio::task::spawn(async move {
                if let Err(err) = http1::Builder::new()
                    .serve_connection(stream, Svc { grpc_client: grpc_client_clone })
                    .await
                {
                    println!("Error serving connection: {:?}", err);
                }
            });
        }
}
