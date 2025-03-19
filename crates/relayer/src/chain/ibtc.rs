use std::sync::{Arc, Mutex};

use super::endpoint::ChainEndpoint;
use super::tracking::TrackedMsgs;
use config::IbtcConfig;
use ibc_relayer_types::core::ics02_client::events::{CreateClient, NewBlock};
use ibc_relayer_types::core::ics02_client::height::Height;
use ibc_service_grpc::ibc_service_grpc_client::IbcServiceGrpcClient;
use ibc_service_grpc::SendIbcMessageRequest;
use penumbra_sdk_proto::box_grpc_svc::BoxGrpcService;
use penumbra_sdk_proto::view::v1::view_service_client::ViewServiceClient;
use penumbra_sdk_proto::view::v1::GasPricesRequest;
use tendermint::time::Time as TmTime;
use tendermint_light_client::verifier::types::LightBlock as TmLightBlock;
use ibc_relayer_types::clients::ics07_tendermint::client_state::ClientState as TmClientState;
use ibc_relayer_types::clients::ics07_tendermint::consensus_state::ConsensusState as TmConsensusState;
use ibc_relayer_types::clients::ics07_tendermint::header::Header as TmHeader;
use tokio::runtime::Runtime as TokioRuntime;
use tracing::{debug, info};
use crate::config::ChainConfig;
use crate::event::IbcEventWithHeight;
use crate::{
    config::Error as ConfigError,
    error::Error,
    keyring::Secp256k1KeyPair,
};

pub mod config;

pub mod ibc_service_grpc {
    tonic::include_proto!("ibc_service_grpc");
}

pub struct IbtcChain {
    config: IbtcConfig,
    rt: Arc<TokioRuntime>,
}

impl ChainEndpoint for IbtcChain {
    type LightBlock = TmLightBlock;
    type Header = TmHeader;
    type ConsensusState = TmConsensusState;
    type ClientState = TmClientState;
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

        Ok(IbtcChain {
            config,
            rt
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
        let runtime = self.rt.clone();

        // Establishes connection to chain
        let mut client = runtime.block_on(
            IbcServiceGrpcClient::connect(self.config.rpc_addr.clone())
        ).unwrap();

        // Sends one message at the time
        for msg in tracked_msgs.messages() {
            let request = tonic::Request::new(
                SendIbcMessageRequest {
                    type_url: msg.type_url.clone(),
                    value: msg.value.clone()
                }
            );
            runtime.block_on(client.send_ibc_message(request));
        }
        
        info!("tracked_msgs: {:?}", tracked_msgs);
        Ok(vec![
            IbcEventWithHeight {
                event: ibc_relayer_types::events::IbcEvent::NewBlock(NewBlock{
                    height: Height::new(1, 1).unwrap()
                }),
                height: Height::new(1, 1).unwrap()
            }
        ])
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
        todo!()
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
        todo!()
    }

    fn query_application_status(&self) -> Result<super::endpoint::ChainStatus, crate::error::Error> {
        todo!()
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
        todo!()
    }

    fn query_consensus_state(
        &self,
        request: super::requests::QueryConsensusStateRequest,
        include_proof: super::requests::IncludeProof,
    ) -> Result<(crate::consensus_state::AnyConsensusState, Option<ibc_relayer_types::core::ics23_commitment::merkle::MerkleProof>), crate::error::Error> {
        todo!()
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
        todo!()
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
        todo!()
    }

    fn build_consensus_state(
        &self,
        light_block: Self::LightBlock,
    ) -> Result<Self::ConsensusState, crate::error::Error> {
        todo!()
    }

    fn build_header(
        &mut self,
        trusted_height: ibc_relayer_types::Height,
        target_height: ibc_relayer_types::Height,
        client_state: &crate::client_state::AnyClientState,
    ) -> Result<(Self::Header, Vec<Self::Header>), crate::error::Error> {
        todo!()
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