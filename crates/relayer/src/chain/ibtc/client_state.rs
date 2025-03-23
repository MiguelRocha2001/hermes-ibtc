use std::time::Duration;

use ibc_relayer_types::core::{ics02_client::{client_state::ClientState, height::Height, trust_threshold::TrustThreshold}, ics23_commitment::commitment::CommitmentRoot, ics24_host::identifier::ChainId};
use serde::{Deserialize, Serialize};

use crate::client_state;

#[derive(Debug)]
pub struct IbtcLightBlock {

}

pub const IBTC_CLIENT_STATE_TYPE_URL: &str = "/ibc.lightclients.tendermint.v1.ClientState";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IbtcClientState {
    pub chain_id: ChainId,
    pub latest_height: Height,
    pub frozen_height: Option<Height>,
    pub trust_threshold: TrustThreshold,
    pub trusting_period: Duration,
    pub max_clock_drift: Duration,
}

impl From<IbtcClientState> for client_state::AnyClientState {
    fn from(value: IbtcClientState) -> Self {
        client_state::AnyClientState::Ibtc(value)
    }
}

impl ClientState for IbtcClientState {
    fn chain_id(&self) -> ibc_relayer_types::core::ics24_host::identifier::ChainId {
        self.chain_id.clone()
    }

    fn client_type(&self) -> ibc_relayer_types::core::ics02_client::client_type::ClientType {
        ibc_relayer_types::core::ics02_client::client_type::ClientType::Ibtc
    }

    fn latest_height(&self) -> ibc_relayer_types::Height {
        self.latest_height
    }

    fn frozen_height(&self) -> Option<ibc_relayer_types::Height> {
        self.frozen_height
    }

    fn expired(&self, elapsed: std::time::Duration) -> bool {
        false
    }
}