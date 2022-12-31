use std::net::SocketAddr;
use tokio::sync::mpsc;
use tonic::{transport::Server, Request, Response, Status};

use protos::httpgrpc::{Header, HttpRequest, HttpResponse};
use protos::httpgrpc::http_server::{Http, HttpServer};

type HttpResult<T> = Result<Response<T>, Status>;

#[derive(Debug)]
pub struct GrpcServer {
    addr: SocketAddr,
}

#[tonic::async_trait]
impl Http for GrpcServer {
    async fn handle(&self, request: Request<HttpRequest>) -> HttpResult<HttpResponse> {
        
        println!("request [{}] from [{}]", request.into_inner().id, self.addr);

        let vec_headers = Header {
            key: "test1".to_owned(),
            values: vec!["1234".to_owned()],
        };
        let vec_headers_2 = Header {
            key: "test2".to_owned(),
            values: vec!["1234".to_owned()],
        };

        Ok(Response::new(HttpResponse { 
            version: "1.1".to_string(), 
            status: 200, 
            headers: vec![vec_headers, vec_headers_2], 
            body: "Pong".as_bytes().to_vec() }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addrs = ["[::1]:50051", "[::1]:50052"];

    let (tx, mut rx) = mpsc::unbounded_channel();

    for addr in &addrs {
        let addr = addr.parse()?;
        let tx = tx.clone();

        let server = GrpcServer { addr };
        let serve = Server::builder()
            .add_service(HttpServer::new(server))
            .serve(addr);

        tokio::spawn(async move {
            if let Err(e) = serve.await {
                eprintln!("Error = {:?}", e);
            }

            tx.send(()).unwrap();
        });
    }

    rx.recv().await;

    Ok(())
}