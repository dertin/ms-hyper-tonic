use uuid::Uuid;

// HTTP/1 server - Hyper.rs
use std::convert::Infallible;
use std::net::SocketAddr;
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn, Service};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

// gRPC protos
use protos::httpgrpc::http_client::HttpClient;
use protos::httpgrpc::{Header, HttpRequest, HttpResponse};

// BlockingClient - Tonic
use std::sync::mpsc::channel;

type StdError = Box<dyn std::error::Error + Send + Sync + 'static>;
type Result<T, E = StdError> = ::std::result::Result<T, E>;

struct BlockingClient {
    client: HttpClient<tonic::transport::Channel>,
    rt: tokio::runtime::Runtime,
}

impl BlockingClient {
    pub fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
    where
        D: TryInto<tonic::transport::Endpoint>,
        D::Error: Into<StdError>,
    {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
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


async fn handle_request(req: Request<Body>, mut grpc_client: HttpClient<tonic::transport::Channel>) -> Result<Response<Body>, hyper::Error> {

    println!(
        "received request! method: {:?}, url: {:?}, headers: {:?}, body: {:?}",
        req.method(),
        req.uri(),
        req.headers(),
        req.body()
    );

    let uuid_request = Uuid::new_v4().to_string();
    let str_method = req.method().to_string();
    let str_uri = req.uri().to_string();
    let str_version = format!("{:?}", req.version());
    let vec_headers = Header { // TODO
        key: "".to_owned(),
        values: vec!["".to_owned()],
    };
    let full_body = hyper::body::to_bytes(req.into_body()).await.unwrap();

    let grpc_request = tonic::Request::new(HttpRequest {
        id: uuid_request,
        version: str_version,
        method: str_method,
        uri: str_uri,
        body: full_body.to_vec(),
        headers: vec![vec_headers],
    });

    // Send message to grpc server

    
    // FIXME: ConnectError("tcp connect error", Os { code: 10048, kind: AddrInUse, message: ...
    // The connection is already in use.
    let response = grpc_client.handle(grpc_request).await.unwrap();
    println!("RESPONSE={:?}", response); // TODO: convert to http response

    /*
    
    // Try blocking but I get the same error.

    let (tx, rx) = channel();

    std::thread::spawn(move || {

        println!("Sending request to gRPC Server...");
        println!("{:?}", grpc_request);

        // FIXME: ConnectError("tcp connect error", Os { code: 10048, kind: AddrInUse, message: ...
        // The connection is already in use.
        let mut grpc_client = BlockingClient::connect("http://[::1]:50051").unwrap();

        let grpc_response = grpc_client.handle(grpc_request).unwrap();
        
        tx.send(grpc_response).unwrap();

    })
    .join()
    .expect("Thread panicked");

    println!("RESPONSE={:?}", rx.recv().unwrap()); // TODO: convert to http response
    */

    Ok(Response::new("Hello, World".into()))
}

#[tokio::main]
async fn main() {
    // We'll bind to 127.0.0.1:3000
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    let endpoints = ["http://[::1]:50051"]
        .iter()
        .map(|a| tonic::transport::Channel::from_static(a));
    let channel_tonic = tonic::transport::Channel::balance_list(endpoints);
    let grpc_client = HttpClient::new(channel_tonic);
    
    // A `Service` is needed for every connection, so this
    // creates one from our `hello_world` function.
    /*let make_svc = make_service_fn(|_conn| async {
        // service_fn converts our function into a `Service`
        Ok::<_, Infallible>(service_fn(handle_request))
    });*/

    let server = Server::bind(&addr).serve(MakeSvc { grpc_client });

    // Run this server for... forever!
    if let Err(e) = server.await {
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
        //println!("{:?}", self.grpc_client);
        Box::pin( { 
            handle_request(req, self.grpc_client.clone())
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
        let mut grpc_client = self.grpc_client.clone();
        let fut = async move { Ok(Svc { grpc_client }) };
        Box::pin(fut)
    }
}