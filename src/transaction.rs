use std::pin::Pin;

use moveit::{moveit, Slot};
use tvl_librocksdb_sys::{
    rocksdb::{PinnableSlice, ReadOptions},
    TransactionWrapper,
};

use crate::{
    into_result, slice::PinnedSlice, DbIterator, Direction, Result, SharedSnapshot, SnapshotRef,
    TransactionDb,
};

pub struct Transaction {
    pub(crate) inner: TransactionWrapper,
    pub(crate) db: TransactionDb,
}

impl Transaction {
    pub fn put(&mut self, col: usize, key: &[u8], value: &[u8]) -> Result<()> {
        let cf = self.db.as_inner().get_cf(col);
        assert!(!cf.is_null());
        moveit! {
            let status = unsafe { self.as_inner_mut().put(cf, &key.into(), &value.into()) };
        }
        into_result(&status)
    }

    pub fn delete(&mut self, col: usize, key: &[u8]) -> Result<()> {
        let cf = self.db.as_inner().get_cf(col);
        assert!(!cf.is_null());
        moveit! {
            let status = unsafe { self.as_inner_mut().del(cf, &key.into()) };
        }
        into_result(&status)
    }

    pub fn get<'a>(
        &'a self,
        col: usize,
        key: &[u8],
        slot: Slot<'a, PinnableSlice>,
    ) -> Result<Option<PinnedSlice<'a>>> {
        moveit! {
            let options = ReadOptions::new();
        }
        self.get_with_options(&options, col, key, slot)
    }

    pub fn get_with_options<'a>(
        &'a self,
        options: &ReadOptions,
        col: usize,
        key: &[u8],
        slot: Slot<'a, PinnableSlice>,
    ) -> Result<Option<PinnedSlice<'a>>> {
        let cf = self.db.as_inner().get_cf(col);
        assert!(!cf.is_null());
        let mut slice = slot.emplace(PinnableSlice::new());
        let slice_ptr = unsafe { slice.as_mut().get_unchecked_mut() };
        moveit! {
            let status = unsafe { self.as_inner().get(options, cf, &key.into(), slice_ptr) };
        }
        if status.IsNotFound() {
            return Ok(None);
        }
        into_result(&status)?;
        Ok(Some(PinnedSlice::new(slice)))
    }

    /// # Panics
    ///
    /// If there are no snapshot set for this transaction.
    pub fn snapshot(&self) -> SnapshotRef<'_> {
        let snap = self.as_inner().snapshot();
        SnapshotRef {
            inner: unsafe { snap.as_ref() }.unwrap(),
            tx: self,
        }
    }

    /// Similar to `snapshot`, but the returned snapshot can outlive the
    /// transaction.
    ///
    /// # Panics
    ///
    /// If there are no snapshot set for this transaction.
    pub fn timestamped_snapshot(&self) -> SharedSnapshot {
        let snap = self.as_inner().timestamped_snapshot();
        assert!(!snap.is_null());
        SharedSnapshot {
            inner: snap,
            db: self.db.clone(),
        }
    }

    pub fn iter(&self, col: usize, dir: Direction) -> DbIterator<&'_ Self> {
        moveit! {
            let options = ReadOptions::new();
        }
        self.iter_with_options(&options, col, dir)
    }

    pub fn iter_with_options<'a>(
        &'a self,
        options: &ReadOptions,
        col: usize,
        dir: Direction,
    ) -> DbIterator<&'a Self> {
        let cf = self.db.as_inner().get_cf(col);
        assert!(!cf.is_null());
        unsafe { DbIterator::new(self.as_inner().iter(options, cf), dir) }
    }

    pub fn rollback(&mut self) -> Result<()> {
        moveit! {
            let status = self.as_inner_mut().rollback();
        }
        into_result(&status)
    }

    pub fn commit(&mut self) -> Result<()> {
        moveit! {
            let status = self.as_inner_mut().commit();
        }
        into_result(&status)
    }

    fn as_inner(&self) -> &TransactionWrapper {
        &self.inner
    }

    fn as_inner_mut(&mut self) -> Pin<&mut TransactionWrapper> {
        Pin::new(&mut self.inner)
    }
}
