use std::net::SocketAddr;
use tokio::sync::mpsc;
use tonic::{transport::Server, Request, Response, Status};

use protos::httpgrpc::{Header, HttpRequest, HttpResponse};

type HttpResult<T> = Result<Response<T>, Status>;

#[derive(Debug)]
pub struct HttpServer {
    addr: SocketAddr,
}

#[tonic::async_trait]
impl protos::httpgrpc::http_server::Http for HttpServer {
    async fn handle(&self, request: Request<HttpRequest>) -> HttpResult<HttpResponse> {
        
        println!("request [{}] from [{}]", request.into_inner().id, self.addr);

        let vec_headers = Header {
            key: "test".to_owned(),
            values: vec!["1234".to_owned()],
        };

        Ok(Response::new(HttpResponse { 
            version: "1.1".to_string(), 
            status: 200, 
            headers: vec![vec_headers], 
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

        let server = HttpServer { addr };
        let serve = Server::builder()
            .add_service(protos::httpgrpc::http_server::HttpServer::new(server))
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