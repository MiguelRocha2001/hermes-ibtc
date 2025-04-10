#![allow(warnings)]

use std::collections::HashSet;
use std::fs;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::endpoint::{ChainEndpoint, ChainStatus};
use super::requests::{IncludeProof, QueryHeight};
use super::tracking::TrackedMsgs;
use config::IbtcConfig;
use http::Uri;
use ibc_proto::ibc::core::client::v1::{QueryClientStateRequest, QueryConsensusStateRequest, QueryConsensusStatesRequest};
use ibc_relayer_types::core::ics02_client;
use ibc_relayer_types::core::ics02_client::events::{CreateClient, NewBlock};
use ibc_relayer_types::core::ics02_client::height::Height;
use ibc_relayer_types::core::ics23_commitment::commitment::CommitmentRoot;
use ibc_relayer_types::timestamp::Timestamp;
use ibc_service_grpc::ibc_service_grpc_client::IbcServiceGrpcClient;
use ibc_proto::ibc::core::connection::v1::QueryConnectionRequest as RawQueryConnectionRequest;
use ibc_service_grpc::{Empty, SendIbcMessageRequest};
use penumbra_sdk_proto::box_grpc_svc::BoxGrpcService;
use penumbra_sdk_proto::view::v1::view_service_client::ViewServiceClient;
use prost::Message;
use serde::Deserialize;
use tendermint::block::{self, Id, Round};
use tendermint::block::signed_header::SignedHeader;
use tendermint::time::Time as TmTime;
use tendermint::validator::Info;
use tendermint_light_client::types::{PeerId, ValidatorSet};
use tendermint_light_client::verifier::types::LightBlock as TmLightBlock;

//use ibc_relayer_types::clients::ics07_tendermint::consensus_state::ConsensusState as TmConsensusState;
//use ibc_relayer_types::clients::ics07_tendermint::client_state::{AllowUpdate, ClientState as TmClientState};

use ibc_relayer_types::clients::ics09_ibtc::client_state::{AllowUpdate, ClientState as IbtcClientState};
use ibc_relayer_types::clients::ics09_ibtc::consensus_state::ConsensusState as IbtcConsensusState;

use ibc_relayer_types::clients::ics09_ibtc::header::Header as IbtcHeader;
use ibc_relayer_types::clients::ics07_tendermint::header::Header as TmHeader;
use tokio::runtime::Runtime as TokioRuntime;
use toml::value::Array;
use tonic::IntoRequest;
use tracing::{debug, info};
use crate::chain::client::ClientSettings;
use crate::chain::penumbra::IBC_PROOF_SPECS;
use crate::config::ChainConfig;
use crate::consensus_state::AnyConsensusState;
use crate::event::{ibc_event_try_from_abci_event, IbcEventWithHeight};
use crate::{
    config::Error as ConfigError,
    error::Error,
    keyring::Secp256k1KeyPair,
};
use ibc_proto::ibc::core::client::v1::QueryClientStateRequest as RawQueryClientStateRequest;
use ibc_proto::ibc::core::commitment::v1::MerkleProof as RawMerkleProof;
use ibc_proto::ibc::core::{
    channel::v1::query_client::QueryClient as IbcChannelQueryClient,
    client::v1::query_client::QueryClient as IbcClientQueryClient,
    connection::v1::query_client::QueryClient as IbcConnectionQueryClient,
};
use crate::client_state::AnyClientState;
use ibc_proto::ibc::core::client::v1::QueryConsensusStateRequest as RawQueryConsensusStatesRequest;
use tendermint::{AppHash, Hash, Time};
use ibc_relayer_types::core::ics02_client::client_state::ClientState;
use ibc_relayer_types::core::ics02_client::client_type::ClientType;
use ibc_relayer_types::core::ics02_client::consensus_state::ConsensusState;
use ibc_relayer_types::core::ics23_commitment::specs::ProofSpecs;
use ibc_relayer_types::core::ics24_host::identifier::ChainId;
use ibc_relayer_types::events::IbcEvent;

use tendermint::abci::Event as AbciEvent;
use ibc_relayer_types::core::ics03_connection::connection::ConnectionEnd;
use ibc_relayer_types::Height as ICSHeight;
use crate::chain::ibtc::ibc_service_grpc::QueryChainHeaderResponse;
// For better understanding of the protocol, read this: https://tutorials.cosmos.network/academy/3-ibc/4-clients.html

// Maybe, we can use these proto structs to interact with IBTC: https://docs.rs/ibc-proto/latest/ibc_proto/ibc/index.html

pub mod config;

pub mod ibc_service_grpc {
    tonic::include_proto!("ibc_service_grpc");
}

pub struct IbtcChain {
    config: IbtcConfig,
    rt: Arc<TokioRuntime>,
    ibc_client_grpc_client: IbcClientQueryClient<tonic::transport::Channel>,
    ibc_connection_grpc_client: IbcConnectionQueryClient<tonic::transport::Channel>,
}

impl IbtcChain {
    fn query_ibtc_header(&self, target_height: Height) -> QueryChainHeaderResponse {
        // Fetch header from IBTC chain.
        let runtime = self.rt.clone();

        // Establishes connection to chain
        let mut client = runtime.block_on(
            IbcServiceGrpcClient::connect(self.config.rpc_addr.clone())
        ).unwrap();

        runtime.block_on(
            client.query_chain_header(tonic::Request::new(ibc_service_grpc::Height {
                revision_number: target_height.revision_number(),
                revision_height: target_height.revision_height(),
            })
            )
        ).unwrap().into_inner()
    }
}

impl ChainEndpoint for IbtcChain {
    type LightBlock = TmLightBlock;
    type Header = IbtcHeader;
    type ConsensusState = IbtcConsensusState;
    type ClientState = IbtcClientState;
    type Time = TmTime;
    // Note: this is a placeholder, we won't actually use it.
    type SigningKeyPair = Secp256k1KeyPair;

    fn id(&self) -> &ibc_relayer_types::core::ics24_host::identifier::ChainId {
        &self.config.id
    }

    fn config(&self) -> crate::config::ChainConfig {
        ChainConfig::Ibtc(self.config.clone())
    }

    fn bootstrap(config: crate::config::ChainConfig, rt: std::sync::Arc<tokio::runtime::Runtime>) -> Result<Self, crate::error::Error> {
        let ChainConfig::Ibtc(config) = config else {
            return Err(Error::config(ConfigError::wrong_type()));
        };

        let grpc_addr = Uri::from_str(&config.rpc_addr.clone())
            .map_err(|e| Error::invalid_uri(config.rpc_addr.clone(), e))?;

        let ibc_client_grpc_client = rt
            .block_on(IbcClientQueryClient::connect(grpc_addr.clone()))
            .map_err(Error::grpc_transport)?;

        let ibc_connection_grpc_client = rt
            .block_on(IbcConnectionQueryClient::connect(grpc_addr.clone()))
            .map_err(Error::grpc_transport)?;

        Ok(IbtcChain {
            config,
            rt,
            ibc_client_grpc_client,
            ibc_connection_grpc_client
        })
    }

    fn shutdown(self) -> Result<(), crate::error::Error> {
        todo!()
    }

    fn health_check(&mut self) -> Result<super::endpoint::HealthCheck, crate::error::Error> {
        todo!()
    }

    fn subscribe(&mut self) -> Result<super::handle::Subscription, crate::error::Error> {
        todo!()
    }

    fn keybase(&self) -> &crate::keyring::KeyRing<Self::SigningKeyPair> {
        todo!()
    }

    fn keybase_mut(&mut self) -> &mut crate::keyring::KeyRing<Self::SigningKeyPair> {
        todo!()
    }

    fn get_signer(&self) -> Result<ibc_relayer_types::signer::Signer, crate::error::Error> {
        Ok(ibc_relayer_types::signer::Signer::dummy())
    }

    fn get_key(&self) -> Result<Self::SigningKeyPair, crate::error::Error> {
        todo!()
    }

    fn version_specs(&self) -> Result<super::version::Specs, crate::error::Error> {
        todo!()
    }

    fn send_messages_and_wait_commit(
        &mut self,
        tracked_msgs: super::tracking::TrackedMsgs,
    ) -> Result<Vec<crate::event::IbcEventWithHeight>, crate::error::Error> {
        /// 1. Submits message to IBTC chain;
        /// 2. Queries IBTC chain for events, and consumes them;
        /// 3. Returns those events

        debug!("send_messages_and_wait_commit(): tracked_msgs={:?}",
            tracked_msgs.msgs.clone().into_iter().map(|msg| msg.type_url).collect::<Vec<String>>()
        );

        let runtime = self.rt.clone();

        // Establishes connection to chain
        let mut client = runtime.block_on(
            IbcServiceGrpcClient::connect(self.config.rpc_addr.clone())
        ).unwrap();

        // Sends one message at the time!
        for msg in tracked_msgs.messages() {
            let request = tonic::Request::new(
                SendIbcMessageRequest {
                    type_url: msg.type_url.clone(),
                    value: msg.value.clone()
                }
            );
            runtime.block_on(client.send_ibc_message(request));
        }

        let request = tonic::Request::new(Empty::default());
        let reply = runtime.block_on(
            client.query_emitted_events(request)
        ).unwrap().into_inner();

        let events: Vec<IbcEventWithHeight> = reply.events
            .iter()
            .map(|value| serde_json::from_slice::<AbciEvent>(&value.value[..]).unwrap())
            .filter_map(|ev| ibc_event_try_from_abci_event(&ev).ok())
            .map(|ev| IbcEventWithHeight::new(ev,
                ICSHeight::new(self.config.id.version(), u64::from(reply.height.unwrap().revision_height)).unwrap())
            )
            .collect();

        debug!("send_messages_and_wait_commit(): events={:?}", events);
        Ok(events)
    }

    fn send_messages_and_wait_check_tx(
        &mut self,
        tracked_msgs: super::tracking::TrackedMsgs,
    ) -> Result<Vec<tendermint_rpc::endpoint::broadcast::tx_sync::Response>, crate::error::Error> {
        todo!()
    }

    fn verify_header(
        &mut self,
        trusted: ibc_relayer_types::Height,
        target: ibc_relayer_types::Height,
        client_state: &crate::client_state::AnyClientState,
    ) -> Result<Self::LightBlock, crate::error::Error> {
        // Called when another chain is creating IBTC LC, after getting the IBTC ClientState (from function build_client_state()).
        // This function queries IBTC for the host state, for a target height, and returns a mock LightBlock, with the host
        // ConsensusState values, for that height: root and time.

        info!("Called verify_header() called with params: trusted={:?}, target={:?} and client_state={:?}", trusted, target, client_state);

        // This is just a template.
        let mock_signed_header_data = fs::read_to_string("crates/relayer-types/tests/support/signed_header.json").unwrap();
        let mut mock_signed_header = serde_json::from_str::<SignedHeader>(&mock_signed_header_data).unwrap();

        // Fetch header from IBTC chain.
        let runtime = self.rt.clone();

        // Establishes connection to chain
        let mut client = runtime.block_on(
            IbcServiceGrpcClient::connect(self.config.rpc_addr.clone())
        ).unwrap();

        let reply = runtime.block_on(
            client.query_chain_header(tonic::Request::new(ibc_service_grpc::Height {
                revision_number: target.revision_number(),
                revision_height: target.revision_height(),
            })
        )).unwrap().into_inner();

        // Affect mock header.
        mock_signed_header.header.time = Time::from_unix_timestamp(reply.time, 0).unwrap();
        mock_signed_header.header.app_hash = AppHash::try_from(reply.root).unwrap();

        Ok(TmLightBlock {
            signed_header: mock_signed_header,
            validators: ValidatorSet::without_proposer(vec![]),
            next_validators: ValidatorSet::without_proposer(vec![]),
            provider: PeerId::new([0; 20])
        })
    }

    fn check_misbehaviour(
        &mut self,
        update: &ibc_relayer_types::core::ics02_client::events::UpdateClient,
        client_state: &crate::client_state::AnyClientState,
    ) -> Result<Option<crate::misbehaviour::MisbehaviourEvidence>, crate::error::Error> {
        todo!()
    }

    fn query_balance(&self, key_name: Option<&str>, denom: Option<&str>) -> Result<crate::account::Balance, crate::error::Error> {
        todo!()
    }

    fn query_all_balances(&self, key_name: Option<&str>) -> Result<Vec<crate::account::Balance>, crate::error::Error> {
        todo!()
    }

    fn query_denom_trace(&self, hash: String) -> Result<crate::denom::DenomTrace, crate::error::Error> {
        todo!()
    }

    fn query_commitment_prefix(&self) -> Result<ibc_relayer_types::core::ics23_commitment::commitment::CommitmentPrefix, crate::error::Error> {
        info!("Called query_commitment_prefix()");

        // Important: must match the one in IBTC IBC module!
        Ok(b"ibc".to_vec().try_into().unwrap())
    }

    fn query_application_status(&self) -> Result<super::endpoint::ChainStatus, crate::error::Error> {
        info!("Called query_application_status()");

        let runtime = self.rt.clone();

        // Establishes connection to chain
        let mut client = runtime.block_on(
            IbcServiceGrpcClient::connect(self.config.rpc_addr.clone())
        ).unwrap();

        let request = tonic::Request::new(Empty::default());
        let reply = runtime.block_on(client.query_application_status(request)).unwrap();

        let reply = reply.into_inner();
        Ok(ChainStatus { 
            // This height is used when hermes calls build_client_state()
            height: Height::new(reply.height.unwrap().revision_number, reply.height.unwrap().revision_height).unwrap(),
            timestamp: Timestamp::from_nanoseconds(reply.timestamp).unwrap()
        })
    }

    fn query_clients(
        &self,
        request: super::requests::QueryClientStatesRequest,
    ) -> Result<Vec<crate::client_state::IdentifiedAnyClientState>, crate::error::Error> {
        todo!()
    }

    fn query_client_state(
        &self,
        request: super::requests::QueryClientStateRequest,
        include_proof: super::requests::IncludeProof,
    ) -> Result<(crate::client_state::AnyClientState, Option<ibc_relayer_types::core::ics23_commitment::merkle::MerkleProof>), crate::error::Error> {
        info!("Called query_client_state(): request={:?}; include_proof={:?}", request, include_proof);

        let mut client = self.ibc_client_grpc_client.clone();

        let height = match request.height {
            QueryHeight::Latest => 0.to_string(),
            QueryHeight::Specific(h) => h.to_string(),
        };

        let proto_request: RawQueryClientStateRequest = request.into();
        let mut request = proto_request.into_request();
        request
            .metadata_mut()
            .insert("height", height.parse().expect("valid height"));

        // TODO(erwan): for now, playing a bit fast-and-loose with the error handling.
        let response: ibc_proto::ibc::core::client::v1::QueryClientStateResponse = self
            .rt
            .block_on(client.client_state(request))
            .map_err(|e| Error::other(e.to_string()))?
            .into_inner();

        let raw_client_state = response
            .client_state
            .ok_or_else(Error::empty_response_value)?;
        let raw_proof_bytes = response.proof;
        // let maybe_proof_height = response.proof_height;

        let client_state: AnyClientState = raw_client_state
            .try_into()
            .map_err(|e: ics02_client::error::Error| Error::other(e.to_string()))?;

            match include_proof {
                IncludeProof::Yes => {
                    // First, check that the raw proof is not empty.
                    if raw_proof_bytes.is_empty() {
                        return Err(Error::empty_response_proof());
                    }
    
                    // Only then, attempt to deserialize the proof.
                    let raw_proof = RawMerkleProof::decode(raw_proof_bytes.as_ref())
                        .map_err(|e| Error::other(e.to_string()))?;
    
                    let proof = raw_proof.into();
    
                    Ok((client_state, Some(proof)))
                }
                IncludeProof::No => Ok((client_state, None)),
            }
    }

    fn query_consensus_state(
        &self,
        request: super::requests::QueryConsensusStateRequest,
        include_proof: super::requests::IncludeProof,
    ) -> Result<(crate::consensus_state::AnyConsensusState, Option<ibc_relayer_types::core::ics23_commitment::merkle::MerkleProof>), crate::error::Error> {
        info!("Called query_consensus_state(): request={:?}; include_proof={:?}", request, include_proof);

        crate::telemetry!(query, self.id(), "query_consensus_state");
        let mut client = self.ibc_client_grpc_client.clone();

        let height: String = match request.query_height {
            QueryHeight::Latest => 0.to_string(),
            QueryHeight::Specific(h) => h.to_string(),
        };

        let mut proto_request: RawQueryConsensusStatesRequest = request.into();
        // TODO(erwan): the connection handshake fails when we request the latest height.
        // This is ostensibly a bug in hermes, in particular when we build the handshake message.
        // However, for now, we can work around this by always overriding the flag to `false`.
        proto_request.latest_height = false;

        let mut request = proto_request.into_request();
        request
            .metadata_mut()
            .insert("height", height.parse().unwrap());
        let response = self
            .rt
            .block_on(client.consensus_state(request))
            .map_err(|e| Error::other(e.to_string()))?
            .into_inner();

        let raw_consensus_state = response
            .consensus_state
            .ok_or_else(Error::empty_response_value)?;
        let raw_proof_bytes = response.proof;

        let consensus_state: AnyConsensusState = raw_consensus_state
            .try_into()
            .map_err(|e: ics02_client::error::Error| Error::other(e.to_string()))?;

        /*
        if !matches!(consensus_state, AnyConsensusState::Ibtc(_)) {
            return Err(Error::consensus_state_type_mismatch(
                ClientType::Ibtc,
                consensus_state.client_type(),
            ));
        }
         */

        match include_proof {
            IncludeProof::No => Ok((consensus_state, None)),
            IncludeProof::Yes => {
                if raw_proof_bytes.is_empty() {
                    return Err(Error::empty_response_proof());
                }

                let raw_proof = RawMerkleProof::decode(raw_proof_bytes.as_ref())
                    .map_err(|e| Error::other(e.to_string()))?;

                let proof = raw_proof.into();

                Ok((consensus_state, Some(proof)))
            }
        }
    }

    fn query_consensus_state_heights(
        &self,
        request: super::requests::QueryConsensusStateHeightsRequest,
    ) -> Result<Vec<ibc_relayer_types::Height>, crate::error::Error> {
        todo!()
    }

    fn query_upgraded_client_state(
        &self,
        request: super::requests::QueryUpgradedClientStateRequest,
    ) -> Result<(crate::client_state::AnyClientState, ibc_relayer_types::core::ics23_commitment::merkle::MerkleProof), crate::error::Error> {
        todo!()
    }

    fn query_upgraded_consensus_state(
        &self,
        request: super::requests::QueryUpgradedConsensusStateRequest,
    ) -> Result<(crate::consensus_state::AnyConsensusState, ibc_relayer_types::core::ics23_commitment::merkle::MerkleProof), crate::error::Error> {
        todo!()
    }

    fn query_connections(
        &self,
        request: super::requests::QueryConnectionsRequest,
    ) -> Result<Vec<ibc_relayer_types::core::ics03_connection::connection::IdentifiedConnectionEnd>, crate::error::Error> {
        todo!()
    }

    fn query_client_connections(
        &self,
        request: super::requests::QueryClientConnectionsRequest,
    ) -> Result<Vec<ibc_relayer_types::core::ics24_host::identifier::ConnectionId>, crate::error::Error> {
        todo!()
    }

    fn query_connection(
        &self,
        request: super::requests::QueryConnectionRequest,
        include_proof: super::requests::IncludeProof,
    ) -> Result<(ibc_relayer_types::core::ics03_connection::connection::ConnectionEnd, Option<ibc_relayer_types::core::ics23_commitment::merkle::MerkleProof>), crate::error::Error> {
        info!("Called query_connection(): request={:?}; include_proof={:?}", request, include_proof);
        //crate::telemetry!(query, self.id(), "query_connection");
        let mut client = self.ibc_connection_grpc_client.clone();

        let height = match request.height {
            QueryHeight::Latest => 0.to_string(),
            QueryHeight::Specific(h) => h.to_string(),
        };
        let connection_id = request.connection_id.clone();

        let proto_request: RawQueryConnectionRequest = request.into();
        let mut request = proto_request.into_request();

        request
            .metadata_mut()
            .insert("height", height.parse().unwrap());

        let response = self.rt.block_on(client.connection(request)).map_err(|e| {
            if e.code() == tonic::Code::NotFound {
                debug!("query_connection(): connection not found: {:?}", Error::connection_not_found(connection_id.clone()));
                Error::connection_not_found(connection_id.clone())
            } else {
                debug!("query_connection(): grpc error: {:?}", Error::grpc_status(e.clone(), "query_connection".to_owned()));
                Error::grpc_status(e, "query_connection".to_owned())
            }
        })?;

        let resp = response.into_inner();
        let connection_end: ConnectionEnd = match resp.connection {
            Some(raw_connection) => raw_connection.try_into().map_err(Error::ics03)?,
            None => {
                // When no connection is found, the GRPC call itself should return
                // the NotFound error code. Nevertheless even if the call is successful,
                // the connection field may not be present, because in protobuf3
                // everything is optional.
                return Err(Error::connection_not_found(connection_id));
            }
        };

        match include_proof {
            IncludeProof::Yes => {
                let raw_proof_bytes = resp.proof;
                if raw_proof_bytes.is_empty() {
                    return Err(Error::empty_response_proof());
                }

                let raw_proof = RawMerkleProof::decode(raw_proof_bytes.as_ref())
                    .map_err(|e| Error::other(e.to_string()))?;

                debug!("Raw connection proof: {:?}", raw_proof);
                let proof = raw_proof.into();
                debug!("Connection proof: {:?}", proof);
                
                Ok((connection_end, Some(proof)))
            },
            IncludeProof::No => Ok((connection_end, None)),
        }
    }

    fn query_connection_channels(
        &self,
        request: super::requests::QueryConnectionChannelsRequest,
    ) -> Result<Vec<ibc_relayer_types::core::ics04_channel::channel::IdentifiedChannelEnd>, crate::error::Error> {
        todo!()
    }

    fn query_channels(
        &self,
        request: super::requests::QueryChannelsRequest,
    ) -> Result<Vec<ibc_relayer_types::core::ics04_channel::channel::IdentifiedChannelEnd>, crate::error::Error> {
        todo!()
    }

    fn query_channel(
        &self,
        request: super::requests::QueryChannelRequest,
        include_proof: super::requests::IncludeProof,
    ) -> Result<(ibc_relayer_types::core::ics04_channel::channel::ChannelEnd, Option<ibc_relayer_types::core::ics23_commitment::merkle::MerkleProof>), crate::error::Error> {
        todo!()
    }

    fn query_channel_client_state(
        &self,
        request: super::requests::QueryChannelClientStateRequest,
    ) -> Result<Option<crate::client_state::IdentifiedAnyClientState>, crate::error::Error> {
        todo!()
    }

    fn query_packet_commitment(
        &self,
        request: super::requests::QueryPacketCommitmentRequest,
        include_proof: super::requests::IncludeProof,
    ) -> Result<(Vec<u8>, Option<ibc_relayer_types::core::ics23_commitment::merkle::MerkleProof>), crate::error::Error> {
        todo!()
    }

    fn query_packet_commitments(
        &self,
        request: super::requests::QueryPacketCommitmentsRequest,
    ) -> Result<(Vec<ibc_relayer_types::core::ics04_channel::packet::Sequence>, ibc_relayer_types::Height), crate::error::Error> {
        todo!()
    }

    fn query_packet_receipt(
        &self,
        request: super::requests::QueryPacketReceiptRequest,
        include_proof: super::requests::IncludeProof,
    ) -> Result<(Vec<u8>, Option<ibc_relayer_types::core::ics23_commitment::merkle::MerkleProof>), crate::error::Error> {
        todo!()
    }

    fn query_unreceived_packets(
        &self,
        request: super::requests::QueryUnreceivedPacketsRequest,
    ) -> Result<Vec<ibc_relayer_types::core::ics04_channel::packet::Sequence>, crate::error::Error> {
        todo!()
    }

    fn query_packet_acknowledgement(
        &self,
        request: super::requests::QueryPacketAcknowledgementRequest,
        include_proof: super::requests::IncludeProof,
    ) -> Result<(Vec<u8>, Option<ibc_relayer_types::core::ics23_commitment::merkle::MerkleProof>), crate::error::Error> {
        todo!()
    }

    fn query_packet_acknowledgements(
        &self,
        request: super::requests::QueryPacketAcknowledgementsRequest,
    ) -> Result<(Vec<ibc_relayer_types::core::ics04_channel::packet::Sequence>, ibc_relayer_types::Height), crate::error::Error> {
        todo!()
    }

    fn query_unreceived_acknowledgements(
        &self,
        request: super::requests::QueryUnreceivedAcksRequest,
    ) -> Result<Vec<ibc_relayer_types::core::ics04_channel::packet::Sequence>, crate::error::Error> {
        todo!()
    }

    fn query_next_sequence_receive(
        &self,
        request: super::requests::QueryNextSequenceReceiveRequest,
        include_proof: super::requests::IncludeProof,
    ) -> Result<(ibc_relayer_types::core::ics04_channel::packet::Sequence, Option<ibc_relayer_types::core::ics23_commitment::merkle::MerkleProof>), crate::error::Error> {
        todo!()
    }

    fn query_txs(&self, request: super::requests::QueryTxRequest) -> Result<Vec<crate::event::IbcEventWithHeight>, crate::error::Error> {
        todo!()
    }

    fn query_packet_events(
        &self,
        request: super::requests::QueryPacketEventDataRequest,
    ) -> Result<Vec<crate::event::IbcEventWithHeight>, crate::error::Error> {
        todo!()
    }

    fn query_host_consensus_state(
        &self,
        request: super::requests::QueryHostConsensusStateRequest,
    ) -> Result<Self::ConsensusState, crate::error::Error> {
        todo!()
    }

    fn build_client_state(
        &self,
        height: ibc_relayer_types::Height,
        settings: super::client::ClientSettings,
    ) -> Result<Self::ClientState, crate::error::Error> {
        // Creates ClientState as the representation of IBTC.
        // Is sent to another chain during creation of IBTC LC.

        info!("Called build_client_state(): height={:?} and settings={:?}", height, settings);
        
        let ClientSettings::Tendermint(settings) = settings;

        let unbonding_period = Duration::new(10*6000, 0);
        let trusting_period_default = unbonding_period * 2/3;
        let trusting_period = settings.trusting_period.unwrap_or(trusting_period_default);

        let proof_specs = IBC_PROOF_SPECS.clone();

        /*
        Ok(Self::ClientState {
            chain_id: self.id().clone(),
            trust_threshold: settings.trust_threshold,
            trusting_period,
            max_clock_drift: settings.max_clock_drift,
            frozen_height: None,
            latest_height: height
        })
        */

        //debug!("{} vs {}",ChainId::new("ibtc".to_string(), 1), self.config.id.clone());

        Ok(IbtcClientState {
            chain_id: self.config.id.clone(),
            trust_threshold: settings.trust_threshold,
            trusting_period: settings.trusting_period.unwrap_or(Duration::new(9990, 0)),
            unbonding_period: Duration::from_secs(999999999),
            max_clock_drift: settings.max_clock_drift,
            latest_height: height,
            proof_specs: ProofSpecs::default(),
            upgrade_path: vec![],
            allow_update: AllowUpdate {
                after_expiry: true,
                after_misbehaviour: true
            },
            frozen_height: None
        })
    }

    fn build_consensus_state(
        &self,
        light_block: Self::LightBlock,
    ) -> Result<Self::ConsensusState, crate::error::Error> {
        // Called after verify_header(), to build a ConsensusState from it.

        info!("Called build_consensus_state() light_block={:?}", light_block);

        /*
        Ok(IbtcConsensusState::new(
            CommitmentRoot::from_bytes(&[10]),
            TmTime::from_unix_timestamp(1744226321, 0).unwrap(),
            Hash::None
        ))
         */

        debug!("Time: {}", light_block.time());

        Ok(IbtcConsensusState::new(
            CommitmentRoot::from_bytes(light_block.signed_header.header.app_hash.as_ref()),
            light_block.time(),
            Hash::None
        ))
    }

    fn build_header(
        &mut self,
        trusted_height: ibc_relayer_types::Height,
        target_height: ibc_relayer_types::Height,
        client_state: &crate::client_state::AnyClientState,
    ) -> Result<(Self::Header, Vec<Self::Header>), crate::error::Error> {
        // This function is called when a connection is being established, in order to update counterparty client.
        // It calls this function to obtain the missing block headers.
        
        // The trusted_height is the one obtained when the client was created.
        // The target_height is the one obtained after querying the status of IBTC.

        // IMPORTANT: I don't know if this is the correct way to build the header.
        // Iam querying IBTC for a header that only has the root and time for a given height
        // (basically, a ConsensusState).
        
        // TODO: verify in chain, not here.

        info!("Called build_header() called: trusted_height={:?}, target_height={:?}, client_state={:?}",
            trusted_height, target_height, client_state);

        let mock_header_file = fs::read_to_string(
            "crates/relayer-types/tests/support/signed_header.json"
        ).unwrap();
        let mut mock_signed_header = serde_json::from_str::<SignedHeader>(&mock_header_file).unwrap();
        debug!("Header header: {:#?}", mock_signed_header);

        let header = self.query_ibtc_header(target_height);

        // Affect mock header.
        mock_signed_header.header.time = Time::from_unix_timestamp(header.time, 0).unwrap();
        mock_signed_header.header.app_hash = AppHash::try_from(header.root).unwrap();
        mock_signed_header.header.height = target_height.into();

        let main_header = IbtcHeader {
            signed_header: mock_signed_header,
            validator_set: ValidatorSet::without_proposer(vec![]),
            trusted_height: target_height,
            trusted_validator_set: ValidatorSet::without_proposer(vec![])
        };

        // --------------------------------------------------------------

        // Constructs supporting headers
        let mut supporting_headers = vec![];
        let mut supporting_height = Height::new(trusted_height.revision_number(), trusted_height.revision_height() + 1).unwrap();
        loop {
            if supporting_height == target_height {
                break
            }
            let header = self.query_ibtc_header(supporting_height);

            let mut mock_signed_header = serde_json::from_str::<SignedHeader>(&mock_header_file).unwrap();
            // Affect mock header.
            mock_signed_header.header.time = Time::from_unix_timestamp(header.time, 0).unwrap();
            mock_signed_header.header.app_hash = AppHash::try_from(header.root).unwrap();
            mock_signed_header.header.height = supporting_height.into();

            supporting_headers.push(IbtcHeader {
                signed_header: mock_signed_header,
                validator_set: ValidatorSet::without_proposer(vec![]),
                trusted_height: Height::new(supporting_height.revision_number(), supporting_height.revision_height()).unwrap(),
                trusted_validator_set: ValidatorSet::without_proposer(vec![])
            });

            supporting_height = Height::new(supporting_height.revision_number(), supporting_height.revision_height() + 1).unwrap();
        }

        //supporting_headers.reverse();
        Ok((main_header, supporting_headers))
    }

    fn maybe_register_counterparty_payee(
        &mut self,
        channel_id: &ibc_relayer_types::core::ics24_host::identifier::ChannelId,
        port_id: &ibc_relayer_types::core::ics24_host::identifier::PortId,
        counterparty_payee: &ibc_relayer_types::signer::Signer,
    ) -> Result<(), crate::error::Error> {
        todo!()
    }

    fn cross_chain_query(
        &self,
        requests: Vec<super::requests::CrossChainQueryRequest>,
    ) -> Result<Vec<ibc_relayer_types::applications::ics31_icq::response::CrossChainQueryResponse>, crate::error::Error> {
        todo!()
    }

    fn query_incentivized_packet(
        &self,
        request: ibc_proto::ibc::apps::fee::v1::QueryIncentivizedPacketRequest,
    ) -> Result<ibc_proto::ibc::apps::fee::v1::QueryIncentivizedPacketResponse, crate::error::Error> {
        todo!()
    }

    fn query_consumer_chains(&self) -> Result<Vec<ibc_relayer_types::applications::ics28_ccv::msgs::ConsumerChain>, crate::error::Error> {
        todo!()
    }

    fn query_upgrade(
        &self,
        request: ibc_proto::ibc::core::channel::v1::QueryUpgradeRequest,
        height: ibc_relayer_types::core::ics02_client::height::Height,
        include_proof: super::requests::IncludeProof,
    ) -> Result<(ibc_relayer_types::core::ics04_channel::upgrade::Upgrade, Option<ibc_relayer_types::core::ics23_commitment::merkle::MerkleProof>), crate::error::Error> {
        todo!()
    }

    fn query_upgrade_error(
        &self,
        request: ibc_proto::ibc::core::channel::v1::QueryUpgradeErrorRequest,
        height: ibc_relayer_types::core::ics02_client::height::Height,
        include_proof: super::requests::IncludeProof,
    ) -> Result<(ibc_relayer_types::core::ics04_channel::upgrade::ErrorReceipt, Option<ibc_relayer_types::core::ics23_commitment::merkle::MerkleProof>), crate::error::Error> {
        todo!()
    }

    fn query_ccv_consumer_id(&self, client_id: ibc_relayer_types::core::ics24_host::identifier::ClientId) -> Result<ibc_relayer_types::applications::ics28_ccv::msgs::ConsumerId, crate::error::Error> {
        todo!()
    }
}