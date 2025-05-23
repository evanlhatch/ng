use crate::debug::TableEntry;
use crate::durability::Durability;
use crate::lru::Lru;
use crate::plumbing::DerivedQueryStorageOps;
use crate::plumbing::LruQueryStorageOps;
use crate::plumbing::QueryFunction;
use crate::plumbing::QueryStorageMassOps;
use crate::plumbing::QueryStorageOps;
use crate::runtime::{FxIndexMap, StampedValue};
use crate::{CycleError, Database, DatabaseKeyIndex, QueryDb, Revision};
use parking_lot::RwLock;
use std::borrow::Borrow;
use std::convert::TryFrom;
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;

mod slot;
use slot::Slot;

/// Memoized queries store the result plus a list of the other queries
/// that they invoked. This means we can avoid recomputing them when
/// none of those inputs have changed.
pub type MemoizedStorage<Q> = DerivedStorage<Q, AlwaysMemoizeValue>;

/// "Dependency" queries just track their dependencies and not the
/// actual value (which they produce on demand). This lessens the
/// storage requirements.
pub type DependencyStorage<Q> = DerivedStorage<Q, NeverMemoizeValue>;

/// Handles storage where the value is 'derived' by executing a
/// function (in contrast to "inputs").
pub struct DerivedStorage<Q, MP>
where
    Q: QueryFunction,
    MP: MemoizationPolicy<Q>,
{
    group_index: u16,
    lru_list: Lru<Slot<Q, MP>>,
    slot_map: RwLock<FxIndexMap<Q::Key, Arc<Slot<Q, MP>>>>,
    policy: PhantomData<MP>,
}

impl<Q, MP> std::panic::RefUnwindSafe for DerivedStorage<Q, MP>
where
    Q: QueryFunction,
    MP: MemoizationPolicy<Q>,
    Q::Key: std::panic::RefUnwindSafe,
    Q::Value: std::panic::RefUnwindSafe,
{
}

pub trait MemoizationPolicy<Q>: Send + Sync
where
    Q: QueryFunction,
{
    fn should_memoize_value(key: &Q::Key) -> bool;

    fn memoized_value_eq(old_value: &Q::Value, new_value: &Q::Value) -> bool;
}

pub enum AlwaysMemoizeValue {}
impl<Q> MemoizationPolicy<Q> for AlwaysMemoizeValue
where
    Q: QueryFunction,
    Q::Value: Eq,
{
    fn should_memoize_value(_key: &Q::Key) -> bool {
        true
    }

    fn memoized_value_eq(old_value: &Q::Value, new_value: &Q::Value) -> bool {
        old_value == new_value
    }
}

pub enum NeverMemoizeValue {}
impl<Q> MemoizationPolicy<Q> for NeverMemoizeValue
where
    Q: QueryFunction,
{
    fn should_memoize_value(_key: &Q::Key) -> bool {
        false
    }

    fn memoized_value_eq(_old_value: &Q::Value, _new_value: &Q::Value) -> bool {
        panic!("cannot reach since we never memoize")
    }
}

impl<Q, MP> DerivedStorage<Q, MP>
where
    Q: QueryFunction,
    MP: MemoizationPolicy<Q>,
{
    fn slot(&self, key: &Q::Key) -> Arc<Slot<Q, MP>> {
        if let Some(v) = self.slot_map.read().get(key) {
            return v.clone();
        }

        let mut write = self.slot_map.write();
        let entry = write.entry(key.clone());
        let key_index = u32::try_from(entry.index()).unwrap();
        let database_key_index = DatabaseKeyIndex {
            group_index: self.group_index,
            query_index: Q::QUERY_INDEX,
            key_index,
        };
        entry
            .or_insert_with(|| Arc::new(Slot::new(key.clone(), database_key_index)))
            .clone()
    }
}

impl<Q, MP> QueryStorageOps<Q> for DerivedStorage<Q, MP>
where
    Q: QueryFunction,
    MP: MemoizationPolicy<Q>,
{
    fn new(group_index: u16) -> Self {
        DerivedStorage {
            group_index,
            slot_map: RwLock::new(FxIndexMap::default()),
            lru_list: Default::default(),
            policy: PhantomData,
        }
    }

    fn fmt_index(
        &self,
        _db: &<Q as QueryDb<'_>>::DynDb,
        index: DatabaseKeyIndex,
        fmt: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        assert_eq!(index.group_index, self.group_index);
        assert_eq!(index.query_index, Q::QUERY_INDEX);
        let slot_map = self.slot_map.read();
        let key = slot_map.get_index(index.key_index as usize).unwrap().0;
        write!(fmt, "{}({:?})", Q::QUERY_NAME, key)
    }

    fn maybe_changed_since(
        &self,
        db: &<Q as QueryDb<'_>>::DynDb,
        input: DatabaseKeyIndex,
        revision: Revision,
    ) -> bool {
        assert_eq!(input.group_index, self.group_index);
        assert_eq!(input.query_index, Q::QUERY_INDEX);
        let slot = self
            .slot_map
            .read()
            .get_index(input.key_index as usize)
            .unwrap()
            .1
            .clone();
        slot.maybe_changed_since(db, revision)
    }

    fn try_fetch(
        &self,
        db: &<Q as QueryDb<'_>>::DynDb,
        key: &Q::Key,
    ) -> Result<Q::Value, CycleError<DatabaseKeyIndex>> {
        db.unwind_if_cancelled();

        let slot = self.slot(key);
        let StampedValue {
            value,
            durability,
            changed_at,
        } = slot.read(db)?;

        if let Some(evicted) = self.lru_list.record_use(&slot) {
            evicted.evict();
        }

        db.salsa_runtime()
            .report_query_read(slot.database_key_index(), durability, changed_at);

        Ok(value)
    }

    fn durability(&self, db: &<Q as QueryDb<'_>>::DynDb, key: &Q::Key) -> Durability {
        self.slot(key).durability(db)
    }

    fn entries<C>(&self, _db: &<Q as QueryDb<'_>>::DynDb) -> C
    where
        C: std::iter::FromIterator<TableEntry<Q::Key, Q::Value>>,
    {
        let slot_map = self.slot_map.read();
        slot_map
            .values()
            .filter_map(|slot| slot.as_table_entry())
            .collect()
    }
}

impl<Q, MP> QueryStorageMassOps for DerivedStorage<Q, MP>
where
    Q: QueryFunction,
    MP: MemoizationPolicy<Q>,
{
    fn purge(&self) {
        self.lru_list.purge();
        *self.slot_map.write() = Default::default();
    }
}

impl<Q, MP> LruQueryStorageOps for DerivedStorage<Q, MP>
where
    Q: QueryFunction,
    MP: MemoizationPolicy<Q>,
{
    fn set_lru_capacity(&self, new_capacity: usize) {
        self.lru_list.set_lru_capacity(new_capacity);
    }
}

impl<Q, MP> DerivedQueryStorageOps<Q> for DerivedStorage<Q, MP>
where
    Q: QueryFunction,
    MP: MemoizationPolicy<Q>,
{
    fn invalidate<S>(&self, db: &mut <Q as QueryDb<'_>>::DynDb, key: &S)
    where
        S: Eq + Hash,
        Q::Key: Borrow<S>,
    {
        db.salsa_runtime_mut()
            .with_incremented_revision(&mut |new_revision| {
                let map_read = self.slot_map.read();

                if let Some(slot) = map_read.get(key) {
                    if let Some(durability) = slot.invalidate(new_revision) {
                        return Some(durability);
                    }
                }

                None
            })
    }
}
