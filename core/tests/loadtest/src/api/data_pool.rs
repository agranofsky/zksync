//! A pool of data required for api tests.

// Built-in uses
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::Arc,
};
// External uses
use rand::{thread_rng, Rng};
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
// Workspace uses
use zksync_types::{tx::TxHash, Address, BlockNumber, PriorityOp, ZkSyncPriorityOp};
// Local uses

// TODO use array deque.

const MAX_LIMIT: usize = 100;
const MAX_QUEUE_LEN: usize = 100;

#[derive(Debug, Default, Clone)]
pub struct AddressData {
    pub txs_count: usize,
    pub ops_count: usize,
}

impl AddressData {
    pub fn gen_txs_offset_limit(&self) -> (usize, usize) {
        let mut rng = thread_rng();

        let offset = rng.gen_range(0, std::cmp::max(1, self.txs_count));
        let limit = rng.gen_range(0, std::cmp::min(MAX_LIMIT, std::cmp::max(offset, 1)));
        (offset, limit)
    }

    pub fn gen_ops_offset_limit(&self) -> (usize, usize) {
        let mut rng = thread_rng();

        let offset = rng.gen_range(0, std::cmp::max(1, self.ops_count));
        let limit = rng.gen_range(0, std::cmp::min(MAX_LIMIT, std::cmp::max(offset, 1)));
        (offset, limit)
    }
}

/// API data pool contents.
#[derive(Debug, Default)]
pub struct ApiDataPoolInner {
    addresses: Vec<Address>,
    data_by_address: HashMap<Address, AddressData>,
    txs: VecDeque<TxHash>,
    priority_ops: VecDeque<PriorityOp>,
    // Blocks with the counter of known transactions in them.
    blocks: BTreeMap<BlockNumber, usize>,
    max_block_number: BlockNumber,
}

impl ApiDataPoolInner {
    pub fn store_address(&mut self, address: Address) -> &mut AddressData {
        self.addresses.push(address);
        self.data_by_address.entry(address).or_default()
    }

    pub fn random_address(&self) -> (Address, &AddressData) {
        let idx = thread_rng().gen_range(0, self.addresses.len());
        let address = self.addresses[idx];
        (address, &self.data_by_address[&address])
    }

    pub fn store_tx_hash(&mut self, address: Address, tx_hash: TxHash) {
        self.txs.push_back(tx_hash);
        if self.txs.len() > MAX_QUEUE_LEN {
            self.txs.pop_front();
        }

        self.store_address(address).txs_count += 1;
    }

    pub fn random_tx_hash(&self) -> TxHash {
        let idx = thread_rng().gen_range(0, self.txs.len());
        self.txs[idx]
    }

    pub fn store_priority_op(&mut self, priority_op: PriorityOp) {
        if let ZkSyncPriorityOp::Deposit(deposit) = &priority_op.data {
            self.store_address(deposit.to).ops_count += 1;
        }

        self.priority_ops.push_back(priority_op);
        if self.priority_ops.len() > MAX_QUEUE_LEN {
            self.priority_ops.pop_front();
        }
    }

    pub fn random_priority_op(&self) -> PriorityOp {
        let idx = thread_rng().gen_range(0, self.priority_ops.len());
        self.priority_ops[idx].clone()
    }

    pub fn store_block(&mut self, number: BlockNumber) {
        self.max_block_number = std::cmp::max(self.max_block_number, number);
        // Update known transactions count in the block.
        *self.blocks.entry(number).or_default() += 1;

        if self.blocks.len() > MAX_QUEUE_LEN {
            // TODO: replace by the pop_first then the `map_first_last` becomes stable.
            let key = *self.blocks.keys().next().unwrap();
            self.blocks.remove(&key);
        }
    }

    pub fn random_block(&self) -> BlockNumber {
        self.random_tx_id().0
    }

    pub fn random_tx_id(&self) -> (BlockNumber, usize) {
        let from = *self.blocks.keys().next().unwrap();
        let to = self.max_block_number;

        let mut rng = thread_rng();
        // Sometimes we have gaps in the block list, so it is not always
        // possible to randomly generate an existing block number.
        for _ in 0..MAX_LIMIT {
            let number = rng.gen_range(from, to + 1);
            if let Some(&block_txs) = self.blocks.get(&number) {
                let tx_id = rng.gen_range(0, block_txs);
                return (number, tx_id);
            }
        }

        unreachable!(
            "Unable to find the appropriate block number after {} attempts.",
            MAX_LIMIT
        );
    }
}

/// Provides needed data for the API load tests.
#[derive(Debug, Clone, Default)]
pub struct ApiDataPool {
    inner: Arc<RwLock<ApiDataPoolInner>>,
}

impl ApiDataPool {
    /// Max limit in the API requests with limit.
    pub const MAX_LIMIT: usize = MAX_LIMIT;

    /// Creates a new pool instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets readonly access to the pool content.
    pub async fn read(&self) -> RwLockReadGuard<'_, ApiDataPoolInner> {
        self.inner.read().await
    }

    /// Gets writeable access to the pool content.
    pub async fn write(&self) -> RwLockWriteGuard<'_, ApiDataPoolInner> {
        self.inner.write().await
    }
}
