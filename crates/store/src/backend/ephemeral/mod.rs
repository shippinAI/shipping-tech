

pub mod blob;
pub mod main;
pub mod read;
pub mod write;

use ahash::AHashMap;
use parking_lot::RwLock;
use std::collections::BTreeMap;

pub struct EphemeralStore {
    pub(crate) state: RwLock<EphemeralState>,
}

pub(crate) struct EphemeralState {
    pub(crate) subspaces: AHashMap<u8, BTreeMap<Vec<u8>, Vec<u8>>>,
}
