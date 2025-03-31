use ibc_proto::google::protobuf::Any;
use ibc_proto::ibc::lightclients::tendermint::v1::ConsensusState as RawConsensusState;
use ibc_proto::Protobuf;
use serde::{Deserialize, Serialize};
use tendermint::{hash::Algorithm, time::Time, Hash};
use tendermint_proto::google::protobuf as tpb;
use ibc_proto::ibc::lightclients::wasm::v1::ConsensusState as WasmConsensusState;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::clients::ics07_tendermint::error::Error;
use crate::clients::ics07_tendermint::header::Header;
use crate::core::ics02_client::client_type::ClientType;
use crate::core::ics02_client::error::Error as Ics02Error;
use crate::core::ics23_commitment::commitment::CommitmentRoot;
use crate::timestamp::Timestamp;

pub const IBTC_CONSENSUS_STATE_TYPE_URL: &str =
    "/ibc.lightclients.wasm.v1.ConsensusState";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusState {
    pub timestamp: Time,
    pub root: CommitmentRoot
}

impl ConsensusState {
    pub fn new(root: CommitmentRoot, timestamp: Time) -> Self {
        Self {
            timestamp,
            root
        }
    }
}

impl crate::core::ics02_client::consensus_state::ConsensusState for ConsensusState {
    fn client_type(&self) -> ClientType {
        ClientType::Tendermint
    }

    fn root(&self) -> &CommitmentRoot {
        &self.root
    }

    fn timestamp(&self) -> Timestamp {
        self.timestamp.into()
    }
}

impl Protobuf<WasmConsensusState> for ConsensusState {}

impl TryFrom<WasmConsensusState> for ConsensusState {
    type Error = Error;

    fn try_from(raw: WasmConsensusState) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl ConsensusState {
    // Method to serialize to base64
    pub fn to_base64(&self) -> Vec<u8> {
        // First serialize to JSON bytes
        let json_bytes = serde_json::to_vec(&self).unwrap();

        // Then encode to base64
        BASE64.encode(json_bytes).as_bytes().to_vec()
    }
}

impl From<ConsensusState> for WasmConsensusState {
    fn from(value: ConsensusState) -> Self {
        // FIXME: shunts like this are necessary due to
        // https://github.com/informalsystems/tendermint-rs/issues/1053
        let tpb::Timestamp { seconds, nanos } = value.timestamp.into();
        let timestamp = ibc_proto::google::protobuf::Timestamp { seconds, nanos };

        WasmConsensusState {
            data: value.to_base64()
        }
    }
}

impl Protobuf<Any> for ConsensusState {}

impl TryFrom<Any> for ConsensusState {
    type Error = Ics02Error;

    fn try_from(raw: Any) -> Result<Self, Self::Error> {
        use bytes::Buf;
        use core::ops::Deref;
        use prost::Message;

        fn decode_consensus_state<B: Buf>(buf: B) -> Result<ConsensusState, Error> {
            WasmConsensusState::decode(buf)
                .map_err(Error::decode)?
                .try_into()
        }

        match raw.type_url.as_str() {
            TENDERMINT_CONSENSUS_STATE_TYPE_URL => {
                decode_consensus_state(raw.value.deref()).map_err(Into::into)
            }
            _ => Err(Ics02Error::unknown_consensus_state_type(raw.type_url)),
        }
    }
}

impl From<ConsensusState> for Any {
    fn from(consensus_state: ConsensusState) -> Self {
        Any {
            type_url: IBTC_CONSENSUS_STATE_TYPE_URL.to_string(),
            value: Protobuf::<WasmConsensusState>::encode_vec(consensus_state),
        }
    }
}

impl From<tendermint::block::Header> for ConsensusState {
    fn from(header: tendermint::block::Header) -> Self {
        Self {
            root: CommitmentRoot::from_bytes(header.app_hash.as_ref()),
            timestamp: header.time
        }
    }
}

impl From<Header> for ConsensusState {
    fn from(header: Header) -> Self {
        Self::from(header.signed_header.header)
    }
}