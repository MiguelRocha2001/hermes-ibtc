use alloc::sync::Arc;
use core::fmt::{Display, Error as FmtError, Formatter};
use core::str::FromStr;
use ibc_relayer::chain::penumbra;
use std::thread;
use tendermint_rpc::Client;

use abscissa_core::application::fatal_error;
use abscissa_core::clap::Parser;
use eyre::eyre;
use itertools::Itertools;
use tendermint_rpc::{client::CompatMode, HttpClient};
use tokio::runtime::Runtime as TokioRuntime;
use tracing::{error, info, instrument};

use ibc_relayer::chain::cosmos::fetch_compat_mode;
use ibc_relayer::chain::handle::Subscription;
use ibc_relayer::config::{ChainConfig, EventSourceMode};
use ibc_relayer::error::Error;
use ibc_relayer::event::source::EventSource;
use ibc_relayer::HERMES_VERSION;
use ibc_relayer_types::core::ics24_host::identifier::ChainId;
use ibc_relayer_types::events::IbcEvent;

use crate::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EventFilter {
    NewBlock,
    Tx,
}

impl EventFilter {
    pub fn matches(&self, event: &IbcEvent) -> bool {
        match self {
            EventFilter::NewBlock => matches!(event, IbcEvent::NewBlock(_)),
            EventFilter::Tx => !(matches!(event, IbcEvent::NewBlock(_) | IbcEvent::ChainError(_))),
        }
    }
}

impl Display for EventFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::NewBlock => write!(f, "NewBlock"),
            Self::Tx => write!(f, "Tx"),
        }
    }
}

impl FromStr for EventFilter {
    type Err = Box<dyn std::error::Error + Send + Sync + 'static>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "NewBlock" => Ok(Self::NewBlock),
            "Tx" => Ok(Self::Tx),
            invalid => Err(format!("unrecognized event type: {invalid}").into()),
        }
    }
}

#[derive(Debug, Parser, PartialEq, Eq)]
pub struct ListenCmd {
    /// Identifier of the chain to listen for events from
    #[clap(
        long = "chain",
        required = true,
        help_heading = "REQUIRED",
        value_name = "CHAIN_ID"
    )]
    chain_id: ChainId,

    /// Add an event type to listen for, can be repeated.
    /// Listen for all events by default (available: Tx, NewBlock).
    #[clap(long = "events", value_name = "EVENT", multiple_values = true)]
    events: Vec<EventFilter>,
}

impl ListenCmd {
    fn cmd(&self) -> eyre::Result<()> {
        let config = app_config();

        let chain_config = config
            .find_chain(&self.chain_id)
            .ok_or_else(|| eyre!("chain '{}' not found in configuration", self.chain_id))?;

        let events = if self.events.is_empty() {
            &[EventFilter::Tx, EventFilter::NewBlock]
        } else {
            self.events.as_slice()
        };

        listen(chain_config, events)
    }
}

impl Runnable for ListenCmd {
    fn run(&self) {
        self.cmd()
            .unwrap_or_else(|e| fatal_error(app_reader(), &*e));
    }
}

/// Listen to events
#[instrument(skip_all, level = "error", fields(chain = %config.id()))]
pub fn listen(config: &ChainConfig, filters: &[EventFilter]) -> eyre::Result<()> {
    let rt = Arc::new(TokioRuntime::new()?);
    let compat_mode = detect_compatibility_mode(config, rt.clone())?;
    let rx = subscribe(config, compat_mode, rt)?;

    while let Ok(event_batch) = rx.recv() {
        match event_batch.as_ref() {
            Ok(batch) => {
                let _span =
                    tracing::error_span!("event_batch", batch_height = %batch.height).entered();

                let matching_events = batch
                    .events
                    .iter()
                    .filter(|e| event_match(&e.event, filters))
                    .collect_vec();

                if matching_events.is_empty() {
                    continue;
                }

                for event in matching_events {
                    info!("{}", event);
                }
            }
            Err(e) => error!("- error: {}", e),
        }
    }

    Ok(())
}

fn event_match(event: &IbcEvent, filters: &[EventFilter]) -> bool {
    filters.iter().any(|f| f.matches(event))
}

fn subscribe(
    chain_config: &ChainConfig,
    compat_mode: CompatMode,
    rt: Arc<TokioRuntime>,
) -> eyre::Result<Subscription> {
    // Q: Should this be restricted only to backends that support it,
    // or are all backends expected to support subscriptions?
    match chain_config {
        ChainConfig::CosmosSdk(config) | ChainConfig::Namada(config) => {
            let (event_source, monitor_tx) = match &config.event_source {
                EventSourceMode::Push { url, batch_delay } => EventSource::websocket(
                    chain_config.id().clone(),
                    url.clone(),
                    compat_mode,
                    *batch_delay,
                    rt,
                ),
                EventSourceMode::Pull {
                    interval,
                    max_retries,
                } => {
                    let mut rpc_client = HttpClient::builder(config.rpc_addr.clone().try_into()?)
                        .user_agent(format!("hermes/{}", HERMES_VERSION))
                        .build()
                        .map_err(|e| Error::rpc(config.rpc_addr.clone(), e))?;
                    rpc_client.set_compat_mode(compat_mode);

                    EventSource::rpc(
                        chain_config.id().clone(),
                        rpc_client,
                        *interval,
                        *max_retries,
                        rt,
                    )
                }
            }?;

            thread::spawn(move || event_source.run());

            let subscription = monitor_tx.subscribe()?;
            Ok(subscription)
        }
        ChainConfig::Penumbra(config) => {
            let (event_source, monitor_tx) = match &config.event_source {
                EventSourceMode::Push { url, batch_delay } => EventSource::websocket(
                    chain_config.id().clone(),
                    url.clone(),
                    compat_mode,
                    *batch_delay,
                    rt,
                ),
                EventSourceMode::Pull {
                    interval,
                    max_retries,
                } => EventSource::rpc(
                    chain_config.id().clone(),
                    HttpClient::new(config.rpc_addr.clone())?,
                    *interval,
                    *max_retries,
                    rt,
                ),
            }?;
            

            thread::spawn(move || event_source.run());

            let subscription = monitor_tx.subscribe()?;
            Ok(subscription)
        }
        ChainConfig::Ibtc(_) => panic!("Not supported for ibtc!")
    }
}

fn detect_compatibility_mode(
    config: &ChainConfig,
    rt: Arc<TokioRuntime>,
) -> eyre::Result<CompatMode> {
    let rpc_addr = match config {
        ChainConfig::CosmosSdk(config) | ChainConfig::Namada(config) => config.rpc_addr.clone(),
        ChainConfig::Penumbra(config) => config.rpc_addr.clone(),
        ChainConfig::Ibtc(_) => panic!("Not supported for ibtc!"),
    };

    let client = HttpClient::builder(rpc_addr.try_into()?)
        .user_agent(format!("hermes/{}", HERMES_VERSION))
        .build()?;

    let compat_mode = match config {
        ChainConfig::CosmosSdk(config) | ChainConfig::Namada(config) => {
            rt.block_on(fetch_compat_mode(&client, config))?
        }
        ChainConfig::Penumbra(config) => {
            let status = rt.block_on(client.status())?;
            penumbra::util::compat_mode_from_version(&config.compat_mode, status.node_info.version)?
        },
        ChainConfig::Ibtc(config) => {
            let status = rt.block_on(client.status())?;
            penumbra::util::compat_mode_from_version(&config.compat_mode, status.node_info.version)?
        }
    };

    Ok(compat_mode)
}

#[cfg(test)]
mod tests {
    use super::{EventFilter, ListenCmd};

    use std::str::FromStr;

    use abscissa_core::clap::Parser;
    use ibc_relayer_types::core::ics24_host::identifier::ChainId;

    #[test]
    fn test_listen_required_only() {
        assert_eq!(
            ListenCmd {
                chain_id: ChainId::from_string("chain_id"),
                events: vec!()
            },
            ListenCmd::parse_from(["test", "--chain", "chain_id"])
        )
    }

    #[test]
    fn test_listen_single_event() {
        assert_eq!(
            ListenCmd {
                chain_id: ChainId::from_string("chain_id"),
                events: vec!(EventFilter::from_str("Tx").unwrap())
            },
            ListenCmd::parse_from(["test", "--chain", "chain_id", "--events", "Tx"])
        )
    }

    #[test]
    fn test_listen_multiple_events() {
        assert_eq!(
            ListenCmd {
                chain_id: ChainId::from_string("chain_id"),
                events: vec!(
                    EventFilter::from_str("Tx").unwrap(),
                    EventFilter::from_str("NewBlock").unwrap()
                )
            },
            ListenCmd::parse_from([
                "test", "--chain", "chain_id", "--events", "Tx", "--events", "NewBlock"
            ])
        )
    }

    #[test]
    fn test_listen_multiple_events_single_flag() {
        assert_eq!(
            ListenCmd {
                chain_id: ChainId::from_string("chain_id"),
                events: vec!(
                    EventFilter::from_str("Tx").unwrap(),
                    EventFilter::from_str("NewBlock").unwrap()
                )
            },
            ListenCmd::parse_from(["test", "--chain", "chain_id", "--events", "Tx", "NewBlock"])
        )
    }

    #[test]
    fn test_listen_unknown_event_filter() {
        assert!(ListenCmd::try_parse_from([
            "test",
            "--chain",
            "chain_id",
            "--events",
            "TestFilter"
        ])
        .is_err())
    }

    #[test]
    fn test_listen_unknown_no_chain() {
        assert!(ListenCmd::try_parse_from(["test"]).is_err())
    }
}
