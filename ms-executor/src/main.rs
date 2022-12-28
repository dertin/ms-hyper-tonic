use tokio::runtime::{Builder, Runtime};
use uuid::Uuid;

// HTTP/1 server - Hyper.rs
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::{body::Incoming as IncomingBody, Request, Response};
use std::net::SocketAddr;
use tokio::net::TcpListener;

// gRPC protos - Tonic
use protos::httpgrpc::http_client::HttpClient;
use protos::httpgrpc::{Header, HttpRequest, HttpResponse};

type StdError = Box<dyn std::error::Error + Send + Sync + 'static>;
type Result<T, E = StdError> = ::std::result::Result<T, E>;

struct BlockingClient {
    client: HttpClient<tonic::transport::Channel>,
    rt: Runtime,
}

impl BlockingClient {
    pub fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
    where
        D: TryInto<tonic::transport::Endpoint>,
        D::Error: Into<StdError>,
    {
        let rt = Builder::new_multi_thread().enable_all().build().unwrap();
        let client = rt.block_on(HttpClient::connect(dst))?;

        Ok(Self { client, rt })
    }

    pub fn handle(
        &mut self,
        request: impl tonic::IntoRequest<HttpRequest>,
    ) -> Result<tonic::Response<HttpResponse>, tonic::Status> {
        self.rt.block_on(self.client.handle(request))
    }
}

#[derive(Debug, Clone)]
struct Svc {
    grpc_client: i32,
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
        //let rt  = Runtime::new().unwrap();

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
        //let str_body = rt.block_on(util_body_to_vec(incoming_request));

        // make gRPC request - http incoming request to grpc outgoing request
        let grpc_request = tonic::Request::new(HttpRequest {
            id: uuid_request,
            version: str_version,
            method: str_method,
            uri: str_uri,
            body: vec![1],
            headers: vec![vec_headers],
        });

        std::thread::spawn(|| {
            println!("Sending request to gRPC Server...");
            let mut grpc_client = BlockingClient::connect("http://[::1]:50051").unwrap();
            let grpc_response = grpc_client.handle(grpc_request).unwrap();
            println!("RESPONSE={:?}", grpc_response);
        })
        .join()
        .expect("Thread panicked");

        Box::pin(async { Ok(Response::new(Full::new(Bytes::from("Hello, World!")))) })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Listener HTTP Server - Hyper.rs
    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    let listener = TcpListener::bind(addr).await?;

    // We start a loop to continuously accept incoming connections

    loop {
        let (stream, _) = listener.accept().await?;

        // Spawn a tokio task to serve multiple connections concurrently
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(stream, Svc { grpc_client: 0 })
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
