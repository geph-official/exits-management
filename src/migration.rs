use std::{collections::BTreeMap, ops::Add, path::Path, str::FromStr};

use geph4_protocol::binder::{
    client::E2eeHttpTransport,
    protocol::{BinderClient, Level},
};
use melstructs::{Address, NetID};
use smol_str::SmolStr;

use crate::{gibbername_hash, ExitInfo};

// production binder
const BINDER_HTTP: &str = "https://binder-v4.geph.io/next-gen";
const BINDER_MASTER_PK: &str = "124526f4e692b589511369687498cce57492bf4da20f8d26019c1cc0c80b6e4b";

pub async fn create_yaml_gibbername(
    yaml_path: &Path,
    wallet_path: &str,
    wallet_addr: Address,
) -> anyhow::Result<String> {
    // get exits list
    let transport = E2eeHttpTransport::new(
        bincode::deserialize(&hex::decode(BINDER_MASTER_PK).expect("invalid hex in binder pk"))?,
        BINDER_HTTP.into(),
        vec![],
    );
    let client = BinderClient(transport);
    let master_summary = client.get_summary().await?;
    let exits: BTreeMap<SmolStr, ExitInfo> = master_summary
        .exits
        .into_iter()
        .map(|e| {
            (
                e.hostname,
                ExitInfo {
                    signing_key: hex::encode(e.signing_key),
                    sosistab_key: hex::encode(e.legacy_direct_sosistab_pk.as_bytes()),
                    country_code: e.country_code,
                    city_code: e.city_code,
                    plus: e.allowed_levels.contains(&Level::Plus),
                },
            )
        })
        .collect();
    let exits_yaml = serde_yaml::to_string(&exits)?;
    std::fs::write(yaml_path, exits_yaml)?;
    let exits_hash = gibbername_hash(exits);

    let melclient = melprot::Client::autoconnect(NetID::Mainnet).await.unwrap();
    let gibbername = gibbername::register(
        &melclient,
        wallet_addr,
        &exits_hash.to_string(),
        wallet_path,
    )
    .await?;
    println!("{gibbername}");
    Ok(gibbername)
}
