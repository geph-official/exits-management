use std::{collections::BTreeMap, env, future::Future, path::Path, str::FromStr, time::Duration};

use anyhow::Context;
use blake3::Hash;
use melstructs::{Address, NetID};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use sqlx::PgPool;
use stdcode::StdcodeSerializeExt;

mod migration;
use migration::*;

const GIBBERNAME: &str = "jermeb-beg";

fn main() {
    smol::block_on(async {
        let yaml_path = Path::new("exits.yaml");
        let wallet_path =
            env::var("WALLET_PATH").expect("Please set the WALLET_PATH environment variable");
        let wallet_addr = Address::from_str(
            &env::var("WALLET_ADDR").expect("Please set the WALLET_ADDR environment variable"),
        )
        .unwrap();

        // update the gibbername binding, retrying on failure
        // it is crucial that we update the gibbername binder *before* updating the database
        repeat_fallible(|| update_gibbername(yaml_path, &wallet_path, wallet_addr)).await;

        // // update the database, retrying on failure
        repeat_fallible(|| update_db(yaml_path)).await;
    })
}

#[derive(Serialize, Deserialize)]
pub struct ExitInfo {
    pub signing_key: String,
    pub sosistab_key: String,
    pub country_code: SmolStr,
    pub city_code: SmolStr,
    pub plus: bool,
    pub user: String,
}

fn get_exits(yaml_path: &Path) -> anyhow::Result<BTreeMap<SmolStr, ExitInfo>> {
    let yaml_str = std::fs::read_to_string(yaml_path)?;
    Ok(serde_yaml::from_str(&yaml_str)?)
}

// what's stored in the gibbername:
// BTreeMap<String, (Vec<u8>, Vec<u8>)>, where
// the key is the hostname, and the value is (signing_key, sosistab_key)
async fn update_gibbername(
    yaml_path: &Path,
    wallet_path: &str,
    wallet_address: Address,
) -> anyhow::Result<()> {
    let exits_hash = gibbername_hash(get_exits(yaml_path)?);
    let melclient = melprot::Client::autoconnect(NetID::Mainnet).await.unwrap();
    gibbername::transfer_name_cmd(
        &melclient,
        GIBBERNAME,
        wallet_path,
        wallet_address,
        &exits_hash.to_string(),
    )
    .await?;
    Ok(())
}

fn gibbername_hash(yaml_exits: BTreeMap<SmolStr, ExitInfo>) -> Hash {
    let exits: BTreeMap<String, _> = yaml_exits
        .iter()
        .map(|(hostname, info)| {
            (
                hostname.as_str().to_owned(),
                (
                    hex::decode(info.signing_key.clone()).unwrap(),
                    hex::decode(info.sosistab_key.clone()).unwrap(),
                ),
            )
        })
        .collect();

    blake3::hash(&exits.stdcode())
}

async fn update_db(yaml_path: &Path) -> anyhow::Result<()> {
    let exits = get_exits(yaml_path)?;
    let database_url =
        env::var("DATABASE_URL").expect("Please set the DATABASE_URL environment variable");
    let pool = PgPool::connect(&database_url).await?;
    let mut tx = pool.begin().await?;

    sqlx::query("delete from exits").execute(&mut tx).await?;

    for (hostname, info) in exits.into_iter() {
        sqlx::query(
            r#"
        INSERT INTO exits VALUES
        ($1, $2, $3, $4, $5, $6)
        "#,
        )
        .bind(hostname.to_string())
        .bind(hex::decode(info.signing_key)?)
        .bind(info.country_code.to_string())
        .bind(info.city_code.to_string())
        .bind(hex::decode(info.sosistab_key)?)
        .bind(info.plus)
        .execute(&mut tx)
        .await
        .context("'o' failed to add exit!! 'o'")?;
    }

    tx.commit().await?;

    Ok(())
}

// Repeats something until it stops failing
async fn repeat_fallible<T, E: std::fmt::Debug, F: Future<Output = Result<T, E>>>(
    mut clos: impl FnMut() -> F,
) -> T {
    loop {
        match clos().await {
            Ok(val) => return val,
            Err(err) => eprintln!("retrying failed: {:?}", err),
        }
        smol::Timer::after(Duration::from_secs(1)).await;
    }
}
