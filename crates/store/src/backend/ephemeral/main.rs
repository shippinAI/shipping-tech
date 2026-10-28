

use super::{EphemeralState, EphemeralStore};
use crate::Store;
use ahash::AHashMap;
use parking_lot::RwLock;
use std::sync::Arc;

impl EphemeralStore {
    pub fn open() -> Store {
        Store::Ephemeral(Arc::new(EphemeralStore {
            state: RwLock::new(EphemeralState {
                subspaces: AHashMap::new(),
            }),
        }))
    }
}
