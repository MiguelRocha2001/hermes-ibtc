use ibc_proto::{google::protobuf::Any, Protobuf};
use ibc_relayer_types::{core::{ics02_client::consensus_state::ConsensusState, ics23_commitment::commitment::CommitmentRoot}, timestamp::Timestamp};
use serde::{Deserialize, Serialize};
use crate::consensus_state;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IbtcConsensusState {
    pub timestamp: Timestamp,
    pub root: CommitmentRoot,
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

impl From<IbtcConsensusState> for consensus_state::AnyConsensusState {
    fn from(value: IbtcConsensusState) -> Self {
        consensus_state::AnyConsensusState::Ibtc(value)
    }
}