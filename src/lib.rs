//! Utility functions for async reactive programming.
//!
//! This crate is intentionally very small as it only provides utilities that
//! are not already found in `futures-util`. It is meant as a supplement, not a
//! replacement for existing
#![no_std]
#![warn(missing_docs)]

use core::{
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures_core::Stream;
use pin_project_lite::pin_project;

/// Extensions to the [`Stream`] trait.
pub trait StreamExt: Stream + Sized {
    /// Deduplicate consecutive identical items.
    ///
    /// To be able to immediately yield items of the underlying stream once it
    /// is produced, but still compare them to the next ones, `Dedup` keeps a
    /// clone of the value that was produced last. If cloning the inner value
    /// is expensive but only part of it is used for comparison, you can use
    /// [`dedup_by_key`][Self::dedup_by_key] as a more efficient alternative.
    fn dedup(self) -> Dedup<Self>
    where
        Self::Item: Clone + PartialEq,
    {
        Dedup::new(self)
    }

    /// Deduplicate consecutive items that the given function produces the same
    /// key for.
    fn dedup_by_key<T, F>(self, key_fn: F) -> DedupByKey<Self, T, F>
    where
        T: PartialEq,
        F: FnMut(&Self::Item) -> T,
    {
        DedupByKey::new(self, key_fn)
    }
}

impl<S: Stream> StreamExt for S {}

pin_project! {
    /// Stream adapter produced by [`StreamExt::dedup`].
    pub struct Dedup<S: Stream> {
        #[pin]
        inner: S,
        prev_item: Option<S::Item>,
    }
}

impl<S: Stream> Dedup<S> {
    fn new(inner: S) -> Self {
        Self { inner, prev_item: None }
    }
}

impl<S> Stream for Dedup<S>
where
    S: Stream,
    S::Item: Clone + PartialEq,
{
    type Item = S::Item;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        let mut this = self.project();
        let next = loop {
            let opt = ready!(this.inner.as_mut().poll_next(cx));
            match opt {
                Some(item) => {
                    if this.prev_item.as_ref() != Some(&item) {
                        *this.prev_item = Some(item.clone());
                        break Some(item);
                    }
                }
                None => break None,
            }
        };
        Poll::Ready(next)
    }
}

pin_project! {
    /// Stream adapter produced by [`StreamExt::dedup_by_key`].
    pub struct DedupByKey<S: Stream, T, F> {
        #[pin]
        inner: S,
        key_fn: F,
        prev_key: Option<T>,
    }
}

impl<S: Stream, T, F> DedupByKey<S, T, F> {
    fn new(inner: S, key_fn: F) -> Self {
        Self { inner, key_fn, prev_key: None }
    }
}

impl<S, T, F> Stream for DedupByKey<S, T, F>
where
    S: Stream,
    T: PartialEq,
    F: FnMut(&S::Item) -> T,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        let mut this = self.project();
        let next = loop {
            let opt = ready!(this.inner.as_mut().poll_next(cx));
            match opt {
                Some(item) => {
                    let key = (this.key_fn)(&item);
                    if this.prev_key.as_ref() != Some(&key) {
                        *this.prev_key = Some(key);
                        break Some(item);
                    }
                }
                None => break None,
            }
        };
        Poll::Ready(next)
    }
}
