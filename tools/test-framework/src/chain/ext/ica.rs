use ibc_relayer::upgrade_chain::requires_legacy_upgrade_proposal;
use serde_json::json;

use ibc_relayer::chain::tracking::TrackedMsgs;
use ibc_relayer_types::applications::ics27_ica::msgs::register::{
    LegacyMsgRegisterInterchainAccount, MsgRegisterInterchainAccount,
};
use ibc_relayer_types::core::ics04_channel::version::Version;
use ibc_relayer_types::events::IbcEvent;
use ibc_relayer_types::tx_msg::Msg;

use crate::chain::cli::ica::{query_interchain_account, register_interchain_account_cli};
use crate::prelude::*;
use crate::types::tagged::*;

pub trait InterchainAccountMethodsExt<Chain> {
    fn register_interchain_account_cli<Counterparty>(
        &self,
        from: &MonoTagged<Chain, &WalletAddress>,
        connection_id: &TaggedConnectionIdRef<Chain, Counterparty>,
    ) -> Result<(), Error>;

    fn query_interchain_account<Counterparty>(
        &self,
        from: &MonoTagged<Chain, &WalletAddress>,
        connection_id: &TaggedConnectionIdRef<Chain, Counterparty>,
    ) -> Result<MonoTagged<Counterparty, WalletAddress>, Error>;
}

impl<Chain: Send> InterchainAccountMethodsExt<Chain> for MonoTagged<Chain, &ChainDriver> {
    fn register_interchain_account_cli<Counterparty>(
        &self,
        from: &MonoTagged<Chain, &WalletAddress>,
        connection_id: &TaggedConnectionIdRef<Chain, Counterparty>,
    ) -> Result<(), Error> {
        let driver = *self.value();
        register_interchain_account_cli(
            driver.chain_id.as_str(),
            &driver.command_path,
            &driver.home_path,
            &driver.rpc_listen_address(),
            from.value().as_str(),
            connection_id.value().as_str(),
        )
    }

    fn query_interchain_account<Counterparty>(
        &self,
        from: &MonoTagged<Chain, &WalletAddress>,
        connection_id: &TaggedConnectionIdRef<Chain, Counterparty>,
    ) -> Result<MonoTagged<Counterparty, WalletAddress>, Error> {
        let driver = *self.value();
        let address = query_interchain_account(
            driver.chain_id.as_str(),
            &driver.command_path,
            &driver.home_path,
            &driver.rpc_listen_address(),
            from.value().as_str(),
            connection_id.value().as_str(),
        )?;

        Ok(MonoTagged::new(WalletAddress(address)))
    }
}

#[allow(clippy::type_complexity)]
pub fn register_unordered_interchain_account<Chain: ChainHandle, Counterparty: ChainHandle>(
    chain: &MonoTagged<Chain, FullNode>,
    handle: &Chain,
    connection: &ConnectedConnection<Chain, Counterparty>,
) -> Result<
    (
        MonoTagged<Chain, Wallet>,
        TaggedChannelId<Chain, Counterparty>,
        TaggedPortId<Chain, Counterparty>,
    ),
    Error,
> {
    let wallet = chain.wallets().relayer().cloned();

    let owner = handle.get_signer()?;

    let version_obj = json!({
        "version": "ics27-1",
        "encoding": "proto3",
        "tx_type": "sdk_multi_msg",
        "controller_connection_id": connection.connection_id_a.0,
        "host_connection_id": connection.connection_id_b.0
    });

    let msg_any =
        if requires_legacy_upgrade_proposal(handle.clone()).map_err(Error::upgrade_chain)? {
            let msg = LegacyMsgRegisterInterchainAccount {
                owner,
                connection_id: connection.connection_id_a.0.clone(),
                version: Version::new(version_obj.to_string()),
            };

            msg.to_any()
        } else {
            let msg = MsgRegisterInterchainAccount {
                owner,
                connection_id: connection.connection_id_a.0.clone(),
                version: Version::new(version_obj.to_string()),
                ordering: Ordering::Unordered,
            };
            msg.to_any()
        };

    let tm = TrackedMsgs::new_static(vec![msg_any], "RegisterInterchainAccount");

    let events = handle
        .send_messages_and_wait_commit(tm)
        .map_err(Error::relayer)?;

    for event in events.iter() {
        if let IbcEvent::OpenInitChannel(open_init) = &event.event {
            let channel_id = open_init.channel_id
                .clone()
                .ok_or(())
                .map_err(|_| Error::generic(eyre!("channel_id is empty in the event response after sending MsgRegisterInterchainAccount")))?;
            return Ok((
                wallet,
                TaggedChannelId::new(channel_id),
                TaggedPortId::new(open_init.port_id.clone()),
            ));
        }
    }

    Err(Error::generic(eyre!("could not retrieve an OpenInitChannel event response after sending MsgRegisterInterchainAccount")))
}

#[allow(clippy::type_complexity)]
pub fn register_ordered_interchain_account<Chain: ChainHandle, Counterparty: ChainHandle>(
    chain: &MonoTagged<Chain, FullNode>,
    handle: &Chain,
    connection: &ConnectedConnection<Chain, Counterparty>,
) -> Result<
    (
        MonoTagged<Chain, Wallet>,
        TaggedChannelId<Chain, Counterparty>,
        TaggedPortId<Chain, Counterparty>,
    ),
    Error,
> {
    let wallet = chain.wallets().relayer().cloned();

    let owner = handle.get_signer()?;

    let version_obj = json!({
        "version": "ics27-1",
        "encoding": "proto3",
        "tx_type": "sdk_multi_msg",
        "controller_connection_id": connection.connection_id_a.0,
        "host_connection_id": connection.connection_id_b.0
    });

    let msg_any =
        if requires_legacy_upgrade_proposal(handle.clone()).map_err(Error::upgrade_chain)? {
            let msg = LegacyMsgRegisterInterchainAccount {
                owner,
                connection_id: connection.connection_id_a.0.clone(),
                version: Version::new(version_obj.to_string()),
            };

            msg.to_any()
        } else {
            let msg = MsgRegisterInterchainAccount {
                owner,
                connection_id: connection.connection_id_a.0.clone(),
                version: Version::new(version_obj.to_string()),
                ordering: Ordering::Ordered,
            };
            msg.to_any()
        };

    let tm = TrackedMsgs::new_static(vec![msg_any], "RegisterInterchainAccount");

    let events = handle
        .send_messages_and_wait_commit(tm)
        .map_err(Error::relayer)?;

    for event in events.iter() {
        if let IbcEvent::OpenInitChannel(open_init) = &event.event {
            let channel_id = open_init.channel_id
                .clone()
                .ok_or(())
                .map_err(|_| Error::generic(eyre!("channel_id is empty in the event response after sending MsgRegisterInterchainAccount")))?;
            return Ok((
                wallet,
                TaggedChannelId::new(channel_id),
                TaggedPortId::new(open_init.port_id.clone()),
            ));
        }
    }

    Err(Error::generic(eyre!("could not retrieve an OpenInitChannel event response after sending MsgRegisterInterchainAccount")))
}
