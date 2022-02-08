use crate::{
    bitcoind::{BitcoinD, BitcoindError},
    db::{fetch_spend_txs_to_broadcast, mark_broadcasted_spend},
};
use jsonrpc::error::{Error, RpcError};
use std::sync::Arc;

pub async fn spend_broadcaster(
    bitcoind: BitcoinD,
    config: Arc<tokio_postgres::Config>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut interval = tokio::time::interval(bitcoind.broadcast_interval);
    loop {
        interval.tick().await;
        log::debug!("Trying to broadcast spend txs...");
        let spend_txs = match fetch_spend_txs_to_broadcast(&config).await {
            Ok(s) => s,
            Err(e) => {
                log::error!("Error while fetching txs: {}", e);
                continue;
            }
        };
        if spend_txs.is_empty() {
            log::debug!("No spend txs in the database");
            continue;
        }
        let results = bitcoind.broadcast_transactions(&spend_txs)?;
        for (result, spend_tx) in results.into_iter().zip(spend_txs) {
            match result {
                Ok(_) | Err(BitcoindError::Server(Error::Rpc(RpcError { code: -27, .. }))) => {
                    // Either it's all good or the tx is already in blockchain,
                    // we mark it as broadcasted and be done
                    mark_broadcasted_spend(&config, &spend_tx.txid()).await?;
                    log::info!("Spend tx '{}' is broadcasted", spend_tx.txid());
                }
                Err(e) => {
                    log::debug!("Error while broadcasting spend {:?}: {}", spend_tx, e);
                }
            }
        }
        log::debug!("Broadcast completed");
    }
}
