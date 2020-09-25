// Copyright (c) 2018-2020 MobileCoin Inc.

//! The public side of mobilecoind-mirror.
//! This program opens two listening ports:
//! 1) A GRPC server for receiving incoming poll requests from the private side of the mirror
//! 2) An http(s) server for receiving client requests which will then be forwarded to the
//!    mobilecoind instance sitting behind the private part of the mirror.

#![feature(decl_macro)]

mod mirror_service;
mod query;
mod utils;

use grpcio::{EnvBuilder, ServerBuilder};
use mc_common::logger::{create_app_logger, log, o, Logger};
use mc_mobilecoind_api::{Empty, GetBlockRequest, GetBlockInfoRequest};
use mc_mobilecoind_json::data_types::{JsonBlockDetailsResponse, JsonLedgerInfoResponse, JsonBlockInfoResponse, JsonProcessedBlockResponse};
use mc_mobilecoind_mirror::{
    mobilecoind_mirror_api::{GetProcessedBlockRequest, QueryRequest, SignedRequest},
    uri::MobilecoindMirrorUri,
};
use mc_util_grpc::{BuildInfoService, ConnectionUriGrpcioServer, HealthService};
use mc_util_uri::{ConnectionUri, Uri, UriScheme};
use mirror_service::MirrorService;
use query::QueryManager;
use rocket::{
    config::{Config as RocketConfig, Environment as RocketEnvironment},
    get,
    http::Status,
    post,
    response::Responder,
    routes, Request, Response,
};
use rocket_contrib::json::Json;
use serde::Deserialize;
use std::sync::Arc;
use structopt::StructOpt;

pub type ClientUri = Uri<ClientUriScheme>;

/// Mobilecoind Mirror Uri Scheme
#[derive(Debug, Hash, Ord, PartialOrd, Eq, PartialEq, Clone)]
pub struct ClientUriScheme {}
impl UriScheme for ClientUriScheme {
    /// The part before the '://' of a URL.
    const SCHEME_SECURE: &'static str = "https";
    const SCHEME_INSECURE: &'static str = "http";

    /// Default port numbers
    const DEFAULT_SECURE_PORT: u16 = 8443;
    const DEFAULT_INSECURE_PORT: u16 = 8000;
}

/// Command line config
#[derive(Clone, Debug, StructOpt)]
#[structopt(
    name = "mobilecoind-mirror-public",
    about = "The public side of mobilecoind-mirror, receiving requests from clients and forwarding them to mobilecoind through the private side of the mirror"
)]
pub struct Config {
    /// Listening URI for the private-public interface connection (GRPC).
    #[structopt(long)]
    pub mirror_listen_uri: MobilecoindMirrorUri,

    /// Listening URI for client requests (HTTP(S)).
    #[structopt(long)]
    pub client_listen_uri: ClientUri,

    /// Override the number of workers used for the client http server.
    /// This controls how many concurrent requests the server can process.
    #[structopt(long)]
    pub num_workers: Option<u16>,

    /// Allow using self-signed TLS certificate for GRPC connections.
    #[structopt(long)]
    pub allow_self_signed_tls: bool,
}

/// State that is accessible by all rocket requests
struct State {
    query_manager: QueryManager,
    logger: Logger,
}

/// Sets the status of the response to 400 (Bad Request).
#[derive(Debug, Clone, PartialEq)]
pub struct BadRequest(pub String);

/// Sets the status code of the response to 400 Bad Request and include an error message in the
/// response.
impl<'r> Responder<'r> for BadRequest {
    fn respond_to(self, req: &Request) -> Result<Response<'r>, Status> {
        let mut build = Response::build();
        build.merge(self.0.respond_to(req)?);

        build.status(Status::BadRequest).ok()
    }
}
impl From<&str> for BadRequest {
    fn from(src: &str) -> Self {
        Self(src.to_owned())
    }
}
impl From<String> for BadRequest {
    fn from(src: String) -> Self {
        Self(src)
    }
}

/// Retreive processed block information.
#[get("/processed-block/<block_num>")]
fn processed_block(
    state: rocket::State<State>,
    block_num: u64,
) -> Result<Json<JsonProcessedBlockResponse>, String> {
    let mut get_processed_block = GetProcessedBlockRequest::new();
    get_processed_block.set_block(block_num);

    let mut query_request = QueryRequest::new();
    query_request.set_get_processed_block(get_processed_block);

    log::debug!(
        state.logger,
        "Enqueueing GetProcessedBlockRequest({})",
        block_num
    );
    let query = state.query_manager.enqueue_query(query_request);
    let query_response = query.wait()?;

    if query_response.has_error() {
        log::error!(
            state.logger,
            "GetProcessedBlockRequest({}) failed: {}",
            block_num,
            query_response.get_error()
        );
        return Err(query_response.get_error().into());
    }
    if !query_response.has_get_processed_block() {
        log::error!(
            state.logger,
            "GetProcessedBlockRequest({}) returned incorrect response type",
            block_num
        );
        return Err("Incorrect response type received".into());
    }

    let response = query_response.get_get_processed_block();
    Ok(Json(JsonProcessedBlockResponse::from(response)))
}

/// Retrieve a single block.
#[get("/ledger/blocks/<block_num>")]
fn block_details(
    state: rocket::State<State>,
    block_num: u64,
) -> Result<Json<JsonBlockDetailsResponse>, String> {
    let mut get_block = GetBlockRequest::new();
    get_block.set_block(block_num);

    let mut query_request = QueryRequest::new();
    query_request.set_get_block(get_block);

    log::debug!(state.logger, "Enqueueing GetBlockRequest({})", block_num);
    let query = state.query_manager.enqueue_query(query_request);
    let query_response = query.wait()?;

    if query_response.has_error() {
        log::error!(
            state.logger,
            "GetBlockRequest({}) failed: {}",
            block_num,
            query_response.get_error()
        );
        return Err(query_response.get_error().into());
    }
    if !query_response.has_get_block() {
        log::error!(
            state.logger,
            "GetBlockRequest({}) returned incorrect response type",
            block_num
        );
        return Err("Incorrect response type received".into());
    }

    log::info!(
        state.logger,
        "GetBlockRequest({}) completed successfully",
        block_num
    );

    let response = query_response.get_get_block();
    Ok(Json(JsonBlockDetailsResponse::from(response)))
}

/// Retrieve a block header
#[get("/ledger/blocks/<block_num>/header")]
fn block_info(
    state: rocket::State<State>,
    block_num: u64,
) -> Result<Json<JsonBlockInfoResponse>, String> {
    let mut get_block_info = GetBlockInfoRequest::new();
    get_block_info.set_block(block_num);

    let mut query_request = QueryRequest::new();
    query_request.set_get_block_info(get_block_info);

    log::debug!(state.logger, "Enqueueing GetBlockInfoRequest({})", block_num);
    let query = state.query_manager.enqueue_query(query_request);
    let query_response = query.wait()?;

    if query_response.has_error() {
        log::error!(
            state.logger,
            "GetBlockInfoRequest({}) failed: {}",
            block_num,
            query_response.get_error()
        );
        return Err(query_response.get_error().into());
    }
    if !query_response.has_get_block_info() {
        log::error!(
            state.logger,
            "GetBlockInfoRequest({}) returned incorrect response type",
            block_num
        );
        return Err("Incorrect response type received".into());
    }

    log::info!(
        state.logger,
        "GetBlockInfoRequest({}) completed successfully",
        block_num
    );

    let response = query_response.get_get_block_info();
    Ok(Json(JsonBlockInfoResponse::from(response)))
}

/// Retrieve ledger information
#[get("/ledger/local")]
fn ledger_info(
    state: rocket::State<State>,
) -> Result<Json<JsonLedgerInfoResponse>, String> {
    let mut query_request = QueryRequest::new();
    query_request.set_get_ledger_info(Empty::new());

    log::debug!(state.logger, "Enqueueing GetLedgerInfo Request");
    let query = state.query_manager.enqueue_query(query_request);
    let query_response = query.wait()?;

    if query_response.has_error() {
        log::error!(
            state.logger,
            "GetLedgerInfoRequest failed: {}",
            query_response.get_error()
        );
        return Err(query_response.get_error().into());
    }
    if !query_response.has_get_ledger_info() {
        log::error!(
            state.logger,
            "GetLedgerInfoRequest returned incorrect response type",
        );
        return Err("Incorrect response type received".into());
    }

    log::info!(
        state.logger,
        "GetLedgerInfoRequest completed successfully"
    );

    let response = query_response.get_get_ledger_info();
    Ok(Json(JsonLedgerInfoResponse::from(response)))
}

#[derive(Deserialize)]
struct JsonSignedRequest {
    request: String,
    signature: Vec<u8>,
}

#[post("/signed-request", format = "json", data = "<request>")]
fn signed_request(
    state: rocket::State<State>,
    request: Json<JsonSignedRequest>,
) -> Result<Vec<u8>, BadRequest> {
    let mut signed_request = SignedRequest::new();
    signed_request.set_json_request(request.request.clone());
    signed_request.set_signature(request.signature.clone());

    let mut query_request = QueryRequest::new();
    query_request.set_signed_request(signed_request);

    log::debug!(
        state.logger,
        "Enqueueing SignedRequest({})",
        request.request,
    );
    let query = state.query_manager.enqueue_query(query_request);
    let query_response = query.wait()?;

    if query_response.has_error() {
        log::error!(
            state.logger,
            "SignedRequest({}) failed: {}",
            request.request,
            query_response.get_error()
        );
        return Err(query_response.get_error().into());
    }
    if !query_response.has_encrypted_response() {
        log::error!(
            state.logger,
            "SignedRequest({}) returned incorrect response type",
            request.request,
        );
        return Err("Incorrect response type received".into());
    }

    log::info!(
        state.logger,
        "SignedRequest({}) completed successfully",
        request.request,
    );

    let response = query_response.get_encrypted_response();
    Ok(response.get_payload().to_vec())
}

// Methods for retrieving ledger info copied from mobilecoind-json

/// Gets information about the entire ledger
// #[get("/ledger/local")]
// fn ledger_info(state: rocket::State<State>) -> Result<Json<JsonLedgerInfoResponse>, String> {
//     let resp = state
//         .mobilecoind_api_client
//         .get_ledger_info(&mc_mobilecoind_api::Empty::new())
//         .map_err(|err| format!("Failed getting ledger info: {}", err))?;
//
//     Ok(Json(JsonLedgerInfoResponse::from(&resp)))
// }

// /// Retrieves the data in a request code
// #[get("/ledger/blocks/<block_num>/header")]
// fn block_info(
//     state: rocket::State<State>,
//     block_num: u64,
// ) -> Result<Json<JsonBlockInfoResponse>, String> {
//     let mut req = mc_mobilecoind_api::GetBlockInfoRequest::new();
//     req.set_block(block_num);
//
//     let resp = state
//         .mobilecoind_api_client
//         .get_block_info(&req)
//         .map_err(|err| format!("Failed getting ledger info: {}", err))?;
//
//     Ok(Json(JsonBlockInfoResponse::from(&resp)))
// }

fn main() {
    mc_common::setup_panic_handler();
    let _sentry_guard = mc_common::sentry::init();

    let config = Config::from_args();
    if !config.allow_self_signed_tls
        && utils::is_tls_self_signed(&config.mirror_listen_uri).expect("is_tls_self_signed failed")
    {
        panic!("Refusing to start with self-signed TLS certificate. Use --allow-self-signed-tls to override this check.");
    }

    let (logger, _global_logger_guard) = create_app_logger(o!());
    log::info!(
        logger,
        "Starting mobilecoind mirror public forwarder, listening for mirror requests on {} and client requests on {}",
        config.mirror_listen_uri.addr(),
        config.client_listen_uri.addr(),
    );

    // Common state.
    let query_manager = QueryManager::default();

    // Start the mirror-facing GRPC server.
    log::info!(logger, "Starting mirror GRPC server");

    let build_info_service = BuildInfoService::new(logger.clone()).into_service();
    let health_service = HealthService::new(None, logger.clone()).into_service();
    let mirror_service = MirrorService::new(query_manager.clone(), logger.clone()).into_service();

    let env = Arc::new(
        EnvBuilder::new()
            .name_prefix("Mirror-RPC".to_string())
            .build(),
    );

    let server_builder = ServerBuilder::new(env)
        .register_service(build_info_service)
        .register_service(health_service)
        .register_service(mirror_service)
        .bind_using_uri(&config.mirror_listen_uri);

    let mut server = server_builder.build().unwrap();
    server.start();

    // Start the client-facing webserver.
    if config.client_listen_uri.use_tls() {
        panic!("Client-listening using TLS is currently not supported due to `ring` crate version compatibility issues.");
    }

    let mut rocket_config = RocketConfig::build(
        RocketEnvironment::active().expect("Failed getitng rocket environment"),
    )
    .address(config.client_listen_uri.host())
    .port(config.client_listen_uri.port());
    if config.client_listen_uri.use_tls() {
        rocket_config = rocket_config.tls(
            config
                .client_listen_uri
                .tls_chain_path()
                .expect("failed getting tls chain path"),
            config
                .client_listen_uri
                .tls_key_path()
                .expect("failed getting tls key path"),
        );
    }
    if let Some(num_workers) = config.num_workers {
        rocket_config = rocket_config.workers(num_workers);
    }
    let rocket_config = rocket_config
        .finalize()
        .expect("Failed creating client http server config");

    log::info!(logger, "Starting client web server");
    rocket::custom(rocket_config)
        .mount("/", routes![processed_block, block_info, block_details, ledger_info, signed_request])
        .manage(State {
            query_manager,
            logger,
        })
        .launch();
}
