//! Utility functions for async reactive programming.
//!
//! This crate is intentionally very small as it only provides utilities that
//! are not already found in `futures-util`. It is meant as a supplement, not a
//! replacement for the existing well-known futures crates.
#![no_std]
#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    semicolon_in_expressions_from_macros,
    unreachable_pub,
    unused_import_braces,
    unused_qualifications,
    clippy::branches_sharing_code,
    clippy::cloned_instead_of_copied,
    clippy::dbg_macro,
    clippy::empty_line_after_outer_attr,
    clippy::inefficient_to_string,
    clippy::macro_use_imports,
    clippy::map_flatten,
    clippy::mod_module_files,
    clippy::mut_mut,
    clippy::nonstandard_macro_braces,
    clippy::semicolon_if_nothing_returned,
    clippy::str_to_string,
    clippy::todo,
    clippy::unreadable_literal,
    clippy::unseparated_literal_suffix,
    clippy::wildcard_imports
)]

use core::{
    mem,
    pin::Pin,
    task::{ready, Context, Poll},
};

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
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

    /// Buffer the items from `self` until `batch_done_stream` produces a value,
    /// and return all buffered values in one batch.
    ///
    /// `batch_done_stream` is polled after all ready items from `self` has been
    /// read.
    ///
    /// Examples for possible `batch_done_stream`s:
    ///
    /// - `futures_channel::mpsc::Receiver<()>`
    /// - `tokio_stream::wrappers::IntervalStream` with its item type mapped to
    ///   `()` using `.map(|_| ())` (`use tokio_stream::StreamExt` for `map`)
    #[cfg(feature = "alloc")]
    fn batch_with<S>(self, batch_done_stream: S) -> BatchWith<Self, S>
    where
        S: Stream<Item = ()>,
    {
        BatchWith::new(self, batch_done_stream)
    }

    /// Flattens a stream of streams by always keeping one inner stream and
    /// yielding its items until the outer stream produces a new inner stream,
    /// at which point the inner stream to yield items from is switched to the
    /// new one.
    ///
    /// Equivalent to RxJS'es
    /// [`switchAll`](https://rxjs.dev/api/index/function/switchAll).
    fn switch(self) -> Switch<Self>
    where
        Self::Item: Stream,
    {
        Switch::new(self)
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
    pub struct DedupByKey<S, T, F> {
        #[pin]
        inner: S,
        key_fn: F,
        prev_key: Option<T>,
    }
}

impl<S, T, F> DedupByKey<S, T, F> {
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

#[cfg(feature = "alloc")]
pin_project! {
    /// Stream adapter produced by [`StreamExt::batch_with`].
    pub struct BatchWith<S1: Stream, S2> {
        #[pin]
        primary_stream: S1,
        #[pin]
        batch_done_stream: S2,
        batch: Vec<S1::Item>,
    }
}

#[cfg(feature = "alloc")]
impl<S1: Stream, S2> BatchWith<S1, S2> {
    fn new(primary_stream: S1, batch_done_stream: S2) -> Self {
        Self { primary_stream, batch_done_stream, batch: Vec::new() }
    }
}

#[cfg(feature = "alloc")]
impl<S1, S2> Stream for BatchWith<S1, S2>
where
    S1: Stream,
    S2: Stream<Item = ()>,
{
    type Item = Vec<S1::Item>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        loop {
            match this.primary_stream.as_mut().poll_next(cx) {
                // Primary stream produced a new item
                Poll::Ready(Some(item)) => this.batch.push(item),
                // Primary stream is closed, don't wait for batch_done_stream
                Poll::Ready(None) => {
                    let has_pending_items = !this.batch.is_empty();
                    return Poll::Ready(has_pending_items.then(|| mem::take(this.batch)));
                }
                // Primary stream is pending (and this task is scheduled for wakeup on new items)
                Poll::Pending => break,
            }
        }

        // Primary stream is pending, check the batch_done_stream
        ready!(this.batch_done_stream.poll_next(cx));

        // batch_done_stream produced an item …
        if this.batch.is_empty() {
            // … but we didn't queue any items from the primary stream.
            Poll::Pending
        } else {
            // … and we have some queued items.
            Poll::Ready(Some(mem::take(this.batch)))
        }
    }
}

pin_project! {
    /// Stream adapter produced by [`StreamExt::switch`].
    pub struct Switch<S: Stream> {
        #[pin]
        outer_stream: S,
        #[pin]
        state: SwitchState<S::Item>,
    }
}

pin_project! {
    #[project = SwitchStateProj]
    enum SwitchState<S> {
        None,
        Some {
            #[pin]
            inner_stream: S,
        }
    }
}

impl<S: Stream> Switch<S> {
    fn new(outer_stream: S) -> Self {
        Self { outer_stream, state: SwitchState::None }
    }
}

impl<S> Stream for Switch<S>
where
    S: Stream,
    S::Item: Stream,
{
    type Item = <S::Item as Stream>::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        let mut outer_stream_closed = false;
        while let Poll::Ready(ready) = this.outer_stream.as_mut().poll_next(cx) {
            match ready {
                Some(inner_stream) => {
                    this.state.set(SwitchState::Some { inner_stream });
                }
                None => {
                    outer_stream_closed = true;
                    break;
                }
            }
        }

        match this.state.project() {
            // No inner stream has been produced yet.
            SwitchStateProj::None => {
                if outer_stream_closed {
                    Poll::Ready(None)
                } else {
                    Poll::Pending
                }
            }
            // An inner stream exists => poll it.
            SwitchStateProj::Some { inner_stream } => match inner_stream.poll_next(cx) {
                // Inner stream produced an item.
                Poll::Ready(Some(item)) => Poll::Ready(Some(item)),
                // Both inner and outer stream are closed.
                Poll::Ready(None) if outer_stream_closed => Poll::Ready(None),
                // Only inner stream is closed, or inner stream is pending.
                Poll::Ready(None) | Poll::Pending => Poll::Pending,
            },
        }
    }
}
