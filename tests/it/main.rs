use async_rx::StreamExt as _;
use futures_util::{stream, FutureExt, StreamExt};
use stream_assert::{assert_closed, assert_next_eq, assert_pending};

mod batch_with;
mod switch;

#[test]
fn dedup_empty() {
    let mut stream = stream::empty::<u8>().dedup();
    assert_closed!(stream);
}

#[test]
fn dedup_one() {
    let mut stream = stream::iter([123]).dedup();
    assert_next_eq!(stream, 123);
    assert_closed!(stream);
}

#[test]
fn actually_dedup() {
    let stream = stream::iter([1, 2, 3, 3, 3, 2, 4, 4]).dedup();
    assert_eq!(stream.collect::<Vec<_>>().now_or_never().unwrap(), vec![1, 2, 3, 2, 4]);
}

#[test]
fn pending() {
    let mut stream = stream::iter([1, 1, 2, 2]).chain(stream::pending()).dedup();
    assert_next_eq!(stream, 1);
    assert_next_eq!(stream, 2);
    assert_pending!(stream);
}

#[test]
fn dedup_by_key() {
    let stream = stream::iter([1, 2, 3, 1, 2, 4, 8]).dedup_by_key(|num| num % 2);
    assert_eq!(stream.collect::<Vec<_>>().now_or_never().unwrap(), vec![1, 2, 3, 2]);
}
