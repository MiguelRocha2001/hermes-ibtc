use core::time::Duration;
use ibc_relayer_types::core::ics02_client::trust_threshold::TrustThreshold;
use ibc_relayer_types::core::ics24_host::identifier::ChainId;
use serde_derive::{Deserialize, Serialize};
use tendermint_rpc::client::CompatMode;
use tendermint_rpc::Url;
use crate::config::{default, EventSourceMode, GenesisRestart, RefreshRate};

use crate::config::PacketFilter;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IbtcConfig {
    pub id: ChainId,

    pub rpc_addr: Url,

    /// Controls which packets will be relayed.
    #[serde(default)]
    pub packet_filter: PacketFilter,

    #[serde(default = "default::max_block_time", with = "humantime_serde")]
    pub max_block_time: Duration,
    
    
    /// A fake key name, not used except to satisfy external interfaces.
    ///
    /// The Config::key_name() method returns &String, forcing configs to own a String.
    /// We don't want to just change the method, because we want our fork to apply cleanly
    /// onto upstream, so we need to
    /// 1. use a stub field (here)
    /// 2. create a separate PR to change the method signature
    /// 3. rebase our change stack once it's merged and remove the stub field.
    #[serde(default)]
    pub stub_key_name: String,

    /// The trust threshold defines what fraction of the total voting power of a known
    /// and trusted validator set is sufficient for a commit to be accepted going forward.
    #[serde(default)]
    pub trust_threshold: TrustThreshold,

    pub clear_interval: Option<u64>,
    
    /// How many packets to fetch at once from the chain when clearing packets
    #[serde(default = "default::query_packets_chunk_size")]
    pub query_packets_chunk_size: usize,

    /// A correction parameter that helps deal with clocks that are only approximately synchronized
    /// between the source and destination chains for a client.
    /// This parameter is used when deciding to accept or reject a new header
    /// (originating from the source chain) for any client with the destination chain
    /// that uses this configuration, unless it is overridden by the client-specific
    /// clock drift option.
    #[serde(default = "default::clock_drift", with = "humantime_serde")]
    pub clock_drift: Duration,

    /// The rate at which to refresh the client referencing this chain,
    /// expressed as a fraction of the trusting period.
    #[serde(default = "default::client_refresh_rate")]
    pub client_refresh_rate: RefreshRate,

    /// The type of event source and associated settings
    pub event_source: EventSourceMode,

    pub compat_mode: Option<CompatMode>,

    // This field is only meant to be set via the `update client` command,
    // for when we need to upgrade a client across a genesis restart and
    // therefore need and archive node to fetch blocks from.
    pub genesis_restart: Option<GenesisRestart>,
}