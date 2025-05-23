use crate::miner_id::check_and_request_miner_id;
use crate::monitor::Monitor;
use crate::sealer::Sealer;
use crate::submitter::Submitter;
use crate::{config::MinerConfig, mine::PoraService, watcher::MineContextWatcher};
use network::NetworkSender;
use std::sync::Arc;
use std::time::Duration;
use storage::config::ShardConfig;
use storage_async::Store;
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub enum MinerMessage {
    /// Enable / Disable Mining
    ToggleMining(bool),

    /// Change mining range
    SetStartPosition(Option<u64>),
    SetEndPosition(Option<u64>),

    /// Change shard config
    SetShardConfig(ShardConfig),
}

pub struct MineService;

impl MineService {
    pub async fn spawn(
        executor: task_executor::TaskExecutor,
        _network_send: NetworkSender,
        config: MinerConfig,
        store: Arc<Store>,
    ) -> Result<broadcast::Sender<MinerMessage>, String> {
        let provider = config.make_provider()?;
        let signing_provider = Arc::new(config.make_signing_provider().await?);

        let (msg_send, msg_recv) = broadcast::channel(1024);

        let miner_id =
            check_and_request_miner_id(&config, store.as_ref(), &signing_provider).await?;
        debug!("miner id setting complete.");

        let mine_context_receiver = MineContextWatcher::spawn(
            executor.clone(),
            msg_recv.resubscribe(),
            provider.clone(),
            &config,
            miner_id,
        );

        let mine_answer_receiver = PoraService::spawn(
            executor.clone(),
            msg_recv.resubscribe(),
            mine_context_receiver.resubscribe(),
            store.clone(),
            &config,
            miner_id,
        );

        Submitter::spawn(
            executor.clone(),
            mine_answer_receiver,
            mine_context_receiver,
            provider.clone(),
            signing_provider,
            store.clone(),
            &config,
        );

        Sealer::spawn(executor.clone(), provider, store, &config, miner_id);

        Monitor::spawn(executor, Duration::from_secs(5));

        debug!("Starting miner service");

        Ok(msg_send)
    }
}
