use ibc_relayer_types::Height;
use tendermint::abci;
use tracing::debug;
use crate::event::{ibc_event_try_from_abci_event, IbcEventWithHeight};

pub mod channel;
pub mod connection;
pub mod fee;
pub mod raw_object;

pub fn from_tx_response_event(height: Height, event: &abci::Event) -> Option<IbcEventWithHeight> {
    //debug!("Event: {:?}", event);
    //debug!("ibc_event_try_from_abci_event(): {:?}", ibc_event_try_from_abci_event(event));
    ibc_event_try_from_abci_event(event)
        .ok()
        .map(|ibc_event| IbcEventWithHeight::new(ibc_event, height))
}
