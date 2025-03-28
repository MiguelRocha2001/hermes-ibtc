use std::time::Duration;
use ibc_proto::ibc;
use ibc_relayer_types::core::{ics02_client::{client_state::ClientState, height::Height, trust_threshold::TrustThreshold}, ics23_commitment::commitment::CommitmentRoot, ics24_host::identifier::ChainId};
use serde::{Deserialize, Serialize};
use ibc_proto::ibc::lightclients::wasm::v1::ClientState as WasmClientState;
use namada_sdk::borsh::BorshSerializeExt;
use tendermint_proto::Protobuf;
use crate::client_state;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

#[derive(Debug)]
pub struct IbtcLightBlock {

}

pub const IBTC_CLIENT_STATE_TYPE_URL: &str = "/ibc.lightclients.wasm.v1.ClientState";
pub const WASM_IBTC_LC_CONTRACT_CHECKSUM: &str = "20768fedff1db6c92eb77bc5453b6ad221a2e37ed1565dd2fd29d54e16ab1980";

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

impl From<WasmClientState> for IbtcClientState {
    fn from(value: WasmClientState) -> Self {
        todo!()
    }
}

impl From<IbtcClientState> for WasmClientState {
    fn from(value: IbtcClientState) -> Self {
        Self {
            data: value.to_base64(),
            checksum: hex::decode(WASM_IBTC_LC_CONTRACT_CHECKSUM).expect("Decoding failed"),
            latest_height: None
        }
    }
}

impl Protobuf<WasmClientState> for IbtcClientState { }

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

impl IbtcClientState {
    // Method to serialize to base64
    pub fn to_base64(&self) -> Vec<u8> {
        // First serialize to JSON bytes
        let json_bytes = serde_json::to_vec(self).unwrap();

        // Then encode to base64
        BASE64.encode(json_bytes).as_bytes().to_vec()
    }
}