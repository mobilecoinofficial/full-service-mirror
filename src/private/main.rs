// Copyright (c) 2018-2020 MobileCoin Inc.

//! The private side of mobilecoind-mirror.
//! This program forms outgoing connections to both a mobilecoind instance, as well as a public
//! mobilecoind-mirror instance. It then proceeds to poll the public side of the mirror for
//! requests which it then forwards to mobilecoind. When a response is received it is then
//! forwarded back to the mirror.

mod crypto;
mod request;

use crate::request::SignedJsonRequest;
use grpcio::{ChannelBuilder, RpcStatus, RpcStatusCode};
use mc_common::logger::{create_app_logger, log, o, Logger};
use mc_mobilecoind_api::{
    external::CompressedRistretto, mobilecoind_api_grpc::MobilecoindApiClient,
};
use mc_mobilecoind_json::data_types::{
    JsonBlockDetailsResponse, JsonBlockIndexByTxPubKeyResponse, JsonBlockInfoResponse,
    JsonLedgerInfoResponse, JsonProcessedBlockResponse,
};
use mc_mobilecoind_mirror::{
    mobilecoind_mirror_api::{UnencryptedResponse, EncryptedResponse, PollRequest, QueryRequest, QueryResponse},
    mobilecoind_mirror_api_grpc::MobilecoindMirrorClient,
    uri::MobilecoindMirrorUri,
};
use mc_util_grpc::ConnectionUriGrpcioChannel;
use rsa::RSAPublicKey;
use std::{
    collections::HashMap, convert::TryFrom, str::FromStr, sync::Arc, thread::sleep, time::Duration,
};
use structopt::StructOpt;

/// A wrapper to ease monitor id parsing from a hex string when using `StructOpt`.
#[derive(Clone, Debug)]
pub struct MonitorId(pub Vec<u8>);
impl FromStr for MonitorId {
    type Err = String;
    fn from_str(src: &str) -> Result<Self, Self::Err> {
        let bytes =
            hex::decode(src).map_err(|err| format!("Error decoding monitor id: {:?}", err))?;
        if bytes.len() != 32 {
            return Err("monitor id needs to be exactly 32 bytes".into());
        }
        Ok(Self(bytes))
    }
}

/// Command line config
#[derive(Clone, Debug, StructOpt)]
#[structopt(
    name = "mobilecoind-mirror-private",
    about = "The private side of mobilecoind-mirror, receiving requests from the public side and forwarding them to mobilecoind"
)]
pub struct Config {
    /// Wallet service URI.
    #[structopt(long, default_value = "http://127.0.0.1:9090/")]
    pub wallet_service_uri: String,

    /// URI for the public side of the mirror.
    #[structopt(long)]
    pub mirror_public_uri: MobilecoindMirrorUri,

    /// How many milliseconds to wait between polling.
    #[structopt(long, default_value = "100", parse(try_from_str=parse_duration_in_milliseconds))]
    pub poll_interval: Duration,

    /// Optional encryption public key. If provided, only signed requests are accepted.
    /// See `example-client.js` for an example on how to submit encrypted requests through
    /// the mirror.
    #[structopt(long, parse(try_from_str=load_public_key))]
    pub mirror_key: Option<RSAPublicKey>,
}

fn main() {
    mc_common::setup_panic_handler();
    let _sentry_guard = mc_common::sentry::init();

    let config = Config::from_args();

    let (logger, _global_logger_guard) = create_app_logger(o!());
    log::info!(
        logger,
        "Starting mobilecoind mirror private forwarder on {}, connecting to wallet service at {}",
        config.mirror_public_uri,
        config.wallet_service_uri,
    );

    // Set up the gRPC connection to the public side of the mirror.
    let mirror_api_client = {
        let env = Arc::new(grpcio::EnvBuilder::new().build());
        let ch = ChannelBuilder::new(env)
            .max_receive_message_len(std::i32::MAX)
            .max_send_message_len(std::i32::MAX)
            .max_reconnect_backoff(Duration::from_millis(2000))
            .initial_reconnect_backoff(Duration::from_millis(1000))
            .connect_to_uri(&config.mirror_public_uri, &logger);

        MobilecoindMirrorClient::new(ch)
    };

    // Main polling loop.
    log::debug!(logger, "Entering main loop");

    let mut pending_responses: HashMap<String, QueryResponse> = HashMap::new();

    loop {
        // Communicate with the public side of the mirror.
        let mut request = PollRequest::new();
        request.set_query_responses(pending_responses.clone());

        log::debug!(
            logger,
            "Calling poll with {} queued responses",
            pending_responses.len()
        );
        match mirror_api_client.poll(&request) {
            Ok(response) => {
                log::debug!(
                    logger,
                    "Poll succeeded, got back {} requests",
                    response.query_requests.len()
                );

                // Clear pending responses since we successfully delivered them to the other side.
                pending_responses.clear();

                // Process requests.
                for (query_id, query_request) in response.query_requests.iter() {
                    let query_logger = logger.new(o!("query_id" => query_id.clone()));

                    let response = {
                        // if let Some(mirror_key) = config.mirror_key.as_ref() {
                            // TODO: update encrypted requests for full-service.
                            // process_encrypted_request(
                            //     &mobilecoind_api_client,
                            //     &monitor_id,
                            //     mirror_key,
                            //     query_request,
                            //     &query_logger,
                            // )
                            // .unwrap_or_else(|err| {
                            //     log::error!(
                            //         query_logger,
                            //         "process_encrypted_request failed: {:?}",
                            //         err
                            //     );

                            //     let mut err_query_response = QueryResponse::new();
                            //     err_query_response.set_error(err.to_string());
                            //     err_query_response
                            // })
                        // } else {
                            process_request(
                                &config.wallet_service_uri,
                                query_request,
                                &query_logger,
                            )
                            .unwrap_or_else(|err| {
                                log::error!(query_logger, "process_request failed: {:?}", err);

                                let mut err_query_response = QueryResponse::new();
                                err_query_response.set_error(err.to_string());
                                err_query_response
                            })
                        // }
                    };

                    pending_responses.insert(query_id.clone(), response);
                }
            }

            Err(err) => {
                log::error!(
                    logger,
                    "Polling the public side of the mirror failed: {:?}",
                    err
                );
            }
        }

        sleep(config.poll_interval);
    }
}

fn process_request(
    wallet_service_uri: &str,
    query_request: &QueryRequest,
    logger: &Logger,
) -> Result<QueryResponse, String> {
    let mut _mirror_response = QueryResponse::new();

    if !query_request.has_unsigned_request() {
        return Err("Only processing unsigned requests".into())
    }

    let unsigned_request = query_request.get_unsigned_request();

    // STUB: Check that the request is of an allowed type.
    // return Err(grpcio::Error::RpcFailure(RpcStatus::new(
    //     RpcStatusCode::INTERNAL,
    //     Some("Unsupported request".into()),
    // )))

    log::debug!(
        logger,
        "Incoming unsigned request ({})",
        unsigned_request.json_request
    );

    // Pass request along to full-service.
    let client = reqwest::blocking::Client::new();
    let res = client
        .post(wallet_service_uri)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(unsigned_request.json_request.clone())
        .send()
        .map_err(|e| e.to_string())?;

    let json_response = res.text().map_err(|e| e.to_string())?;

    let mut unencrypted_response = UnencryptedResponse::new();
    unencrypted_response.set_json_response(json_response);

    let mut mirror_response = QueryResponse::new();
    mirror_response.set_unencrypted_response(unencrypted_response);
    Ok(mirror_response)
}


fn process_encrypted_request(
    mobilecoind_api_client: &MobilecoindApiClient,
    monitor_id: &[u8],
    mirror_key: &RSAPublicKey,
    query_request: &QueryRequest,
    logger: &Logger,
) -> grpcio::Result<QueryResponse> {
    if !query_request.has_signed_request() {
        return Err(grpcio::Error::RpcFailure(RpcStatus::new(
            RpcStatusCode::INTERNAL,
            Some("Only processing signed requests".into()),
        )));
    }

    let signed_request = query_request.get_signed_request();

    log::debug!(
        logger,
        "Incoming signed request ({})",
        signed_request.json_request
    );
    let sig_is_valid = crypto::verify_sig(
        mirror_key,
        signed_request.json_request.as_bytes(),
        &signed_request.signature,
    )
    .is_ok();

    if !sig_is_valid {
        let mut err_query_response = QueryResponse::new();
        err_query_response.set_error("Signature verification failed".to_owned());
        return Ok(err_query_response);
    }

    let json_request: SignedJsonRequest = match serde_json::from_str(&signed_request.json_request) {
        Ok(req) => req,
        Err(err) => {
            let mut err_query_response = QueryResponse::new();
            err_query_response.set_error(format!("Error parsing JSON request: {}", err));
            return Ok(err_query_response);
        }
    };

    let json_response_result = match json_request {
        SignedJsonRequest::GetProcessedBlock { block } => {
            let mut mobilecoind_request = mc_mobilecoind_api::GetProcessedBlockRequest::new();
            mobilecoind_request.set_monitor_id(monitor_id.to_vec());
            mobilecoind_request.set_block(block);

            log::debug!(
                logger,
                "Incoming get_processed_block({}, {}), forwarding to mobilecoind",
                hex::encode(monitor_id),
                block
            );
            let mobilecoind_response =
                mobilecoind_api_client.get_processed_block(&mobilecoind_request)?;
            log::info!(
                logger,
                "get_processed_block({}, {}) succeeded",
                hex::encode(monitor_id),
                block,
            );

            serde_json::to_vec(&JsonProcessedBlockResponse::from(&mobilecoind_response))
        }

        SignedJsonRequest::GetBlock { block } => {
            let mut mobilecoind_request = mc_mobilecoind_api::GetBlockRequest::new();
            mobilecoind_request.set_block(block);

            log::debug!(
                logger,
                "Incoming get_block({}), forwarding to mobilecoind",
                block
            );
            let mobilecoind_response = mobilecoind_api_client.get_block(&mobilecoind_request)?;
            log::info!(logger, "get_block({}) succeeded", block);

            serde_json::to_vec(&JsonBlockDetailsResponse::from(&mobilecoind_response))
        }

        SignedJsonRequest::GetBlockInfo { block } => {
            let mut mobilecoind_request = mc_mobilecoind_api::GetBlockInfoRequest::new();
            mobilecoind_request.set_block(block);

            log::debug!(
                logger,
                "Incoming get_block_info({}), forwarding to mobilecoind",
                block
            );
            let mobilecoind_response =
                mobilecoind_api_client.get_block_info(&mobilecoind_request)?;
            log::info!(logger, "get_block_info({}) succeeded", block);

            serde_json::to_vec(&JsonBlockInfoResponse::from(&mobilecoind_response))
        }

        SignedJsonRequest::GetLedgerInfo => {
            log::debug!(
                logger,
                "Incoming get_ledger_info(), forwarding to mobilecoind",
            );
            let mobilecoind_response =
                mobilecoind_api_client.get_ledger_info(&mc_mobilecoind_api::Empty::new())?;
            log::info!(logger, "get_ledger_info() succeeded");

            serde_json::to_vec(&JsonLedgerInfoResponse::from(&mobilecoind_response))
        }

        SignedJsonRequest::GetBlockIndexByTxPubKey { tx_public_key } => {
            log::debug!(
                logger,
                "Incoming get_block_index_by_tx_pubkey({}), forwarding to mobilecoind",
                tx_public_key
            );
            let tx_out_public_key = hex::decode(&tx_public_key).map_err(|err| {
                grpcio::Error::RpcFailure(RpcStatus::new(
                    RpcStatusCode::INTERNAL,
                    Some(format!("Failed to decode hex public key: {}", err)),
                ))
            })?;

            let mut tx_out_public_key_proto = CompressedRistretto::new();
            tx_out_public_key_proto.set_data(tx_out_public_key);

            let mut mobilecoind_request = mc_mobilecoind_api::GetBlockIndexByTxPubKeyRequest::new();
            mobilecoind_request.set_tx_public_key(tx_out_public_key_proto);

            let mobilecoind_response =
                mobilecoind_api_client.get_block_index_by_tx_pub_key(&mobilecoind_request)?;
            log::info!(
                logger,
                "get_block_index_by_tx_pubkey({}) succeeded",
                tx_public_key
            );

            serde_json::to_vec(&JsonBlockIndexByTxPubKeyResponse::from(
                &mobilecoind_response,
            ))
        }
    };
    let json_response = json_response_result.map_err(|err| {
        grpcio::Error::RpcFailure(RpcStatus::new(
            RpcStatusCode::INTERNAL,
            Some(format!("json serialization error: {}", err)),
        ))
    })?;

    let encrypted_payload = crypto::encrypt(mirror_key, &json_response).map_err(|_err| {
        grpcio::Error::RpcFailure(RpcStatus::new(
            RpcStatusCode::INTERNAL,
            Some("Encryption failed".into()),
        ))
    })?;

    let mut encrypted_response = EncryptedResponse::new();
    encrypted_response.set_payload(encrypted_payload);

    let mut mirror_response = QueryResponse::new();
    mirror_response.set_encrypted_response(encrypted_response);
    Ok(mirror_response)
}

fn parse_duration_in_milliseconds(src: &str) -> Result<Duration, std::num::ParseIntError> {
    Ok(Duration::from_millis(u64::from_str(src)?))
}

fn load_public_key(src: &str) -> Result<RSAPublicKey, String> {
    let key_str = std::fs::read_to_string(src)
        .map_err(|err| format!("failed reading key file {}: {:?}", src, err))?;
    let pem = pem::parse(&key_str)
        .map_err(|err| format!("failed parsing key file {}: {:?}", src, err))?;
    Ok(RSAPublicKey::try_from(pem)
        .map_err(|err| format!("failed loading key file {}: {:?}", src, err))?)
}
