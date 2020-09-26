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
    external::CompressedRistretto, mobilecoind_api_grpc::MobilecoindApiClient, MobilecoindUri,
};
use mc_mobilecoind_json::data_types::{
    JsonBlockDetailsResponse, JsonBlockIndexByTxPubKeyResponse, JsonBlockInfoResponse,
    JsonLedgerInfoResponse, JsonProcessedBlockResponse,
};
use mc_mobilecoind_mirror::{
    mobilecoind_mirror_api::{EncryptedResponse, PollRequest, QueryRequest, QueryResponse},
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
    /// MobileCoinD URI.
    #[structopt(long, default_value = "insecure-mobilecoind://127.0.0.1/")]
    pub mobilecoind_uri: MobilecoindUri,

    /// URI for the public side of the mirror.
    #[structopt(long)]
    pub mirror_public_uri: MobilecoindMirrorUri,

    /// How many milliseconds to wait between polling.
    #[structopt(long, default_value = "100", parse(try_from_str=parse_duration_in_milliseconds))]
    pub poll_interval: Duration,

    /// Monitor id to operate with. If not provided, mobilecoind will be queried and if it has only
    /// one monitor id that one would be automatically chosen.
    #[structopt(long)]
    pub monitor_id: Option<MonitorId>,

    /// Optional encryption public key. If provided, all request types but SignedRequest are
    /// disabled. See `example-client.js` for an example on how to submit encrypted requests
    /// through the mirror.
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
        "Starting mobilecoind mirror private forwarder on {}, connecting to mobilecoind {}",
        config.mirror_public_uri,
        config.mobilecoind_uri,
    );

    // Set up the gRPC connection to the mobilecoind client
    let mobilecoind_api_client = {
        let env = Arc::new(grpcio::EnvBuilder::new().build());
        let ch = ChannelBuilder::new(env)
            .max_receive_message_len(std::i32::MAX)
            .max_send_message_len(std::i32::MAX)
            .connect_to_uri(&config.mobilecoind_uri, &logger);

        MobilecoindApiClient::new(ch)
    };

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

    // Figure out which monitor id we are working with.
    let monitor_id = config.monitor_id.map(|m| m.0).unwrap_or_else(|| {
        let response = mobilecoind_api_client.get_monitor_list(&mc_mobilecoind_api::Empty::new()).expect("Failed querying mobilecoind for list of configured monitors");
        match response.monitor_id_list.len() {
            0 => panic!("Mobilecoind has no monitors configured"),
            1 => response.monitor_id_list[0].to_vec(),
            _ => {
                let monitor_ids = response.get_monitor_id_list().iter().map(hex::encode).collect::<Vec<_>>();
                panic!("Mobilecoind has more than one configured monitor, use --monitor-id to select which one to use. The following monitor ids were reported: {:?}", monitor_ids);
            }
    }});
    log::info!(logger, "Monitor id: {}", hex::encode(&monitor_id));

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
                        if let Some(mirror_key) = config.mirror_key.as_ref() {
                            process_encrypted_request(
                                &mobilecoind_api_client,
                                &monitor_id,
                                mirror_key,
                                query_request,
                                &query_logger,
                            )
                            .unwrap_or_else(|err| {
                                log::error!(
                                    query_logger,
                                    "process_encrypted_request failed: {:?}",
                                    err
                                );

                                let mut err_query_response = QueryResponse::new();
                                err_query_response.set_error(err.to_string());
                                err_query_response
                            })
                        } else {
                            process_request(
                                &mobilecoind_api_client,
                                &monitor_id,
                                query_request,
                                &query_logger,
                            )
                            .unwrap_or_else(|err| {
                                log::error!(query_logger, "process_request failed: {:?}", err);

                                let mut err_query_response = QueryResponse::new();
                                err_query_response.set_error(err.to_string());
                                err_query_response
                            })
                        }
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
    mobilecoind_api_client: &MobilecoindApiClient,
    monitor_id: &[u8],
    query_request: &QueryRequest,
    logger: &Logger,
) -> grpcio::Result<QueryResponse> {
    let mut mirror_response = QueryResponse::new();

    // GetProcessedBlock
    if query_request.has_get_processed_block() {
        let mirror_request = query_request.get_get_processed_block();
        let mut mobilecoind_request = mc_mobilecoind_api::GetProcessedBlockRequest::new();
        mobilecoind_request.set_monitor_id(monitor_id.to_vec());
        mobilecoind_request.set_block(mirror_request.block);

        log::debug!(
            logger,
            "Incoming get_processed_block({}, {}), forwarding to mobilecoind",
            hex::encode(monitor_id),
            mirror_request.block
        );
        let mobilecoind_response =
            mobilecoind_api_client.get_processed_block(&mobilecoind_request)?;
        log::info!(
            logger,
            "get_processed_block({}, {}) succeeded",
            hex::encode(monitor_id),
            mirror_request.block,
        );

        mirror_response.set_get_processed_block(mobilecoind_response);
        return Ok(mirror_response);
    }

    // GetBlockRequest
    if query_request.has_get_block() {
        let mirror_request = query_request.get_get_block();

        log::debug!(
            logger,
            "Incoming get_block({}), forwarding to mobilecoind",
            mirror_request.block
        );
        let mobilecoind_response = mobilecoind_api_client.get_block(mirror_request)?;
        log::info!(logger, "get_block({}) succeeded", mirror_request.block);

        mirror_response.set_get_block(mobilecoind_response);
        return Ok(mirror_response);
    }

    // GetBlockInfoRequest
    if query_request.has_get_block_info() {
        let mirror_request = query_request.get_get_block_info();

        log::debug!(
            logger,
            "Incoming get_block_info({}), forwarding to mobilecoind",
            mirror_request.block
        );
        let mobilecoind_response = mobilecoind_api_client.get_block_info(mirror_request)?;
        log::info!(logger, "get_block_info({}) succeeded", mirror_request.block);

        mirror_response.set_get_block_info(mobilecoind_response);
        return Ok(mirror_response);
    }

    // GetLedgerInfoRequest
    if query_request.has_get_ledger_info() {
        log::debug!(
            logger,
            "Incoming get_ledger_info, forwarding to mobilecoind",
        );
        let mobilecoind_response =
            mobilecoind_api_client.get_ledger_info(&mc_mobilecoind_api::Empty::new())?;
        log::info!(logger, "get_ledger_info succeeded");

        mirror_response.set_get_ledger_info(mobilecoind_response);
        return Ok(mirror_response);
    }

    // GetBlockIndexByTxPubKeyRequest
    if query_request.has_get_block_index_by_tx_pub_key() {
        let mirror_request = query_request.get_get_block_index_by_tx_pub_key();

        log::debug!(
            logger,
            "Incoming get_block_index_by_tx_pub_key({}), forwarding to mobilecoind",
            hex::encode(mirror_request.get_tx_public_key().get_data())
        );
        let mobilecoind_response =
            mobilecoind_api_client.get_block_index_by_tx_pub_key(mirror_request)?;
        log::info!(
            logger,
            "Incoming get_block_index_by_tx_pub_key({}) succeeded",
            hex::encode(mirror_request.get_tx_public_key().get_data())
        );

        mirror_response.set_get_block_index_by_tx_pub_key(mobilecoind_response);
        return Ok(mirror_response);
    }

    // Unknown response.
    Err(grpcio::Error::RpcFailure(RpcStatus::new(
        RpcStatusCode::INTERNAL,
        Some("Unsupported request".into()),
    )))
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
