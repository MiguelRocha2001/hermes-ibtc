use std::time::Duration;

use prost::Message;
use serde::{Deserialize, Serialize};

use ibc_proto::google::protobuf::Any;
use ibc_proto::ibc::core::client::v1::Height as RawHeight;
use ibc_proto::Protobuf;

use tendermint_light_client_verifier::options::Options;
use ibc_proto::ibc::lightclients::wasm::v1::ClientState as WasmClientState;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use subtle_encoding::hex;
use crate::clients::ics07_tendermint::error::Error;
use crate::clients::ics07_tendermint::header::Header as TmHeader;
use crate::core::ics02_client::client_state::{
    ClientState as Ics2ClientState, UpgradableClientState,
};
use crate::core::ics02_client::client_type::ClientType;
use crate::core::ics02_client::error::Error as Ics02Error;
use crate::core::ics02_client::trust_threshold::TrustThreshold;
use crate::core::ics23_commitment::specs::ProofSpecs;
use crate::core::ics24_host::identifier::ChainId;
use crate::timestamp::{Timestamp, ZERO_DURATION};
use crate::Height;

pub const IBTC_WASM_CLIENT_STATE_TYPE_URL: &str = "/ibc.lightclients.wasm.v1.ClientState";

// TODO: read checksum from file to avoid unnecessary compiling
pub const WASM_IBTC_LC_CONTRACT_CHECKSUM: &str = "fc93f4a6606904f609e2789a9a46d1606c547293b7cda7bd0840d2c5289423bd";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClientState {
    pub chain_id: ChainId,
    pub trust_threshold: TrustThreshold,
    pub trusting_period: Duration,
    pub unbonding_period: Duration,
    pub max_clock_drift: Duration,
    pub latest_height: Height,
    pub proof_specs: ProofSpecs,
    pub upgrade_path: Vec<String>,
    pub allow_update: AllowUpdate,
    pub frozen_height: Option<Height>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowUpdate {
    pub after_expiry: bool,
    pub after_misbehaviour: bool,
}

impl ClientState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        chain_id: ChainId,
        trust_threshold: TrustThreshold,
        trusting_period: Duration,
        unbonding_period: Duration,
        max_clock_drift: Duration,
        latest_height: Height,
        proof_specs: ProofSpecs,
        upgrade_path: Vec<String>,
        allow_update: AllowUpdate,
    ) -> Result<ClientState, Error> {
        // Basic validation of trusting period and unbonding period: each should be non-zero.
        if trusting_period <= Duration::new(0, 0) {
            return Err(Error::invalid_trusting_period(format!(
                "ClientState trusting period ({trusting_period:?}) must be greater than zero"
            )));
        }

        if unbonding_period <= Duration::new(0, 0) {
            return Err(Error::invalid_unbonding_period(format!(
                "ClientState unbonding period ({unbonding_period:?}) must be greater than zero"
            )));
        }

        if trusting_period >= unbonding_period {
            return Err(Error::invalid_trusting_period(format!(
                "ClientState trusting period ({trusting_period:?}) must be smaller than unbonding period ({unbonding_period:?})",
            )));
        }

        // `TrustThreshold` is guaranteed to be in the range `[0, 1)`,
        // but a zero value is invalid in this context.
        if trust_threshold.numerator() == 0 {
            return Err(Error::validation(
                "ClientState trust threshold cannot be zero".to_string(),
            ));
        }

        // Dividing by zero is undefined so we also rule out a zero denominator.
        // This should be checked already by the `TrustThreshold` constructor
        // but it does not hurt to redo the check here.
        if trust_threshold.denominator() == 0 {
            return Err(Error::validation(
                "ClientState trust threshold cannot divide by zero".to_string(),
            ));
        }

        // Disallow empty proof-specs
        if proof_specs.is_empty() {
            return Err(Error::validation(
                "ClientState proof specs cannot be empty".to_string(),
            ));
        }

        Ok(Self {
            chain_id,
            trust_threshold,
            trusting_period,
            unbonding_period,
            max_clock_drift,
            latest_height,
            proof_specs,
            upgrade_path,
            allow_update,
            frozen_height: None,
        })
    }

    pub fn latest_height(&self) -> Height {
        self.latest_height
    }

    pub fn with_header(self, h: TmHeader) -> Result<Self, Error> {
        Ok(ClientState {
            latest_height: Height::new(
                self.latest_height.revision_number(),
                h.signed_header.header.height.into(),
            )
            .map_err(|_| Error::invalid_header_height(h.signed_header.header.height.value()))?,
            ..self
        })
    }

    pub fn with_frozen_height(self, h: Height) -> Result<Self, Error> {
        Ok(Self {
            frozen_height: Some(h),
            ..self
        })
    }

    /// Helper method to produce a [`Options`] struct for use in
    /// Tendermint-specific light client verification.
    pub fn as_light_client_options(&self) -> Options {
        Options {
            trust_threshold: self.trust_threshold.into(),
            trusting_period: self.trusting_period,
            clock_drift: self.max_clock_drift,
        }
    }

    /// Verify the time and height delays
    pub fn verify_delay_passed(
        current_time: Timestamp,
        current_height: Height,
        processed_time: Timestamp,
        processed_height: Height,
        delay_period_time: Duration,
        delay_period_blocks: u64,
    ) -> Result<(), Error> {
        let earliest_time =
            (processed_time + delay_period_time).map_err(Error::timestamp_overflow)?;
        if !(current_time == earliest_time || current_time.after(&earliest_time)) {
            return Err(Error::not_enough_time_elapsed(current_time, earliest_time));
        }

        let earliest_height = processed_height + delay_period_blocks;
        if current_height < earliest_height {
            return Err(Error::not_enough_blocks_elapsed(
                current_height,
                earliest_height,
            ));
        }

        Ok(())
    }

    /// Verify that the client is at a sufficient height and unfrozen at the given height
    pub fn verify_height(&self, height: Height) -> Result<(), Error> {
        if self.latest_height < height {
            return Err(Error::insufficient_height(self.latest_height(), height));
        }

        match self.frozen_height {
            Some(frozen_height) if frozen_height <= height => {
                Err(Error::client_frozen(frozen_height, height))
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpgradeOptions {
    pub unbonding_period: Duration,
}

impl Ics2ClientState for ClientState {
    fn chain_id(&self) -> ChainId {
        self.chain_id.clone()
    }

    fn client_type(&self) -> ClientType {
        ClientType::Tendermint
    }

    fn latest_height(&self) -> Height {
        self.latest_height
    }

    fn frozen_height(&self) -> Option<Height> {
        self.frozen_height
    }

    fn expired(&self, elapsed: Duration) -> bool {
        elapsed > self.trusting_period
    }
}

impl UpgradableClientState for ClientState {
    type UpgradeOptions = UpgradeOptions;

    fn upgrade(
        &mut self,
        upgrade_height: Height,
        upgrade_options: UpgradeOptions,
        chain_id: ChainId,
    ) {
        // Reset custom fields to zero values
        self.trusting_period = ZERO_DURATION;
        self.trust_threshold = TrustThreshold::CLIENT_STATE_RESET;
        self.allow_update.after_expiry = false;
        self.allow_update.after_misbehaviour = false;
        self.frozen_height = None;
        self.max_clock_drift = ZERO_DURATION;

        // Upgrade the client state
        self.latest_height = upgrade_height;
        self.unbonding_period = upgrade_options.unbonding_period;
        self.chain_id = chain_id;
    }
}

impl Protobuf<WasmClientState> for ClientState {}

impl TryFrom<WasmClientState> for ClientState {
    type Error = Error;

    fn try_from(raw: WasmClientState) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl From<ClientState> for WasmClientState {
    fn from(value: ClientState) -> Self {
        let json = serde_json::to_vec(&value).unwrap();
        let encoded = BASE64.encode(json).encode_to_vec();

        Self {
            data: encoded,
            checksum: hex::decode(WASM_IBTC_LC_CONTRACT_CHECKSUM).expect("Decoding failed"),
            latest_height: Some(ibc_proto::ibc::core::client::v1::Height::from(
                value.latest_height.clone()
            ))
        }
    }
}

impl Protobuf<Any> for ClientState {}

impl TryFrom<Any> for ClientState {
    type Error = Ics02Error;

    fn try_from(raw: Any) -> Result<Self, Self::Error> {
        use bytes::Buf;
        use core::ops::Deref;

        fn decode_client_state<B: Buf>(buf: B) -> Result<ClientState, Error> {
            WasmClientState::decode(buf)
                .map_err(Error::decode)?
                .try_into()
        }

        match raw.type_url.as_str() {
            IBTC_WASM_CLIENT_STATE_TYPE_URL => {
                decode_client_state(raw.value.deref()).map_err(Into::into)
            }
            _ => Err(Ics02Error::unexpected_client_state_type(
                IBTC_WASM_CLIENT_STATE_TYPE_URL.to_string(),
                raw.type_url,
            )),
        }
    }
}

impl From<ClientState> for Any {
    fn from(client_state: ClientState) -> Self {
        Any {
            type_url: IBTC_WASM_CLIENT_STATE_TYPE_URL.to_string(),
            value: Protobuf::<WasmClientState>::encode_vec(client_state),
        }
    }
}