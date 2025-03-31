/*
use ibc_proto::{google::protobuf::Any, Protobuf};
use ibc_relayer_types::{core::{ics02_client::consensus_state::ConsensusState, ics23_commitment::commitment::CommitmentRoot}, timestamp::Timestamp};
use serde::{Deserialize, Serialize};
use crate::consensus_state;
use ibc_proto::ibc::lightclients::wasm::v1::ConsensusState as WasmConsensusState;
use crate::chain::ibtc::client_state::IbtcClientState;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use tracing::log::debug;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IbtcConsensusState {
    pub timestamp: Timestamp,
    pub root: CommitmentRoot,
}

impl From<IbtcConsensusState> for consensus_state::AnyConsensusState {
    fn from(value: IbtcConsensusState) -> Self {
        consensus_state::AnyConsensusState::Ibtc(value)
    }
}

impl ibc_proto::Protobuf<WasmConsensusState> for IbtcConsensusState { }

impl From<IbtcConsensusState> for WasmConsensusState {
    fn from(value: IbtcConsensusState) -> Self {
        Self {
            data: value.to_base64()
        }
    }
}

impl From<WasmConsensusState> for IbtcConsensusState {
    fn from(value: WasmConsensusState) -> Self {
        todo!()
    }
}

impl ConsensusState for IbtcConsensusState {
    fn client_type(&self) -> ibc_relayer_types::core::ics02_client::client_type::ClientType {
        todo!()
    }

    fn root(&self) -> &CommitmentRoot {
        todo!()
    }

    fn timestamp(&self) -> Timestamp {
        todo!()
    }
}

impl IbtcConsensusState {
    // Method to serialize to base64
    pub fn to_base64(&self) -> Vec<u8> {
        // First serialize to JSON bytes
        let json_bytes = serde_json::to_vec(&self).unwrap();

        debug!("{:?}", String::from_utf8(json_bytes.clone()));

        // Then encode to base64
        BASE64.encode(json_bytes).as_bytes().to_vec()
    }
}
 */