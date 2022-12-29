use self::multiplex_service::MultiplexService;
use axum::{routing::get, Router};

use protos::httpgrpc::http_server::{Http, HttpServer};
use protos::httpgrpc::{Header, HttpRequest, HttpResponse};

use std::net::SocketAddr;
use tonic::{Response as TonicResponse, Status};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod multiplex_service;

#[derive(Default)]
struct GrpcServiceImpl {}

#[tonic::async_trait]
impl Http for GrpcServiceImpl {
    async fn handle(
        &self,
        request: tonic::Request<HttpRequest>,
    ) -> Result<TonicResponse<HttpResponse>, Status> {
        tracing::info!("Got a request from {:?}", request.remote_addr());

        let request_clone = request.into_inner();

        let reply = HttpResponse {
            status: 200,
            version: request_clone.version,
            headers: request_clone.headers,
            body: request_clone.body,
        };

        Ok(TonicResponse::new(reply))
    }
}

async fn web_root() -> &'static str {
    "Hello, World!"
}

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_rest_grpc_multiplex=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // build the rest service
    let rest = Router::new().route("/", get(web_root));

    // build the grpc service
    let grpc = HttpServer::new(GrpcServiceImpl::default());

    // combine them into one service
    let service = MultiplexService::new(rest, grpc);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(tower::make::Shared::new(service))
        .await
        .unwrap();
}