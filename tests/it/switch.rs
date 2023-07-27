use std::{pin::pin, slice};

use async_rx::StreamExt as _;
use futures_util::{
    future::Either,
    stream::{self, empty},
};
use stream_assert::{assert_closed, assert_next_eq, assert_pending};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

#[test]
fn empty_switch() {
    let empty: stream::Empty<stream::Iter<slice::Iter<u8>>> = empty();
    let mut empty_switch = empty.switch();
    assert_closed!(empty_switch);
}

#[test]
fn switch_single_outer_stream() {
    let mut stream = pin!(stream::once(async { stream::iter([1, 2, 3]) }).switch());
    assert_next_eq!(stream, 1);
    assert_next_eq!(stream, 2);
    assert_next_eq!(stream, 3);
    assert_closed!(stream);
}

#[test]
fn switch_immediately() {
    let mut stream = pin!(stream::iter([
        stream::iter([1, 2, 3]),
        stream::iter([4, 5, 6]),
        stream::iter([7, 8, 9]),
    ])
    .switch());

    assert_next_eq!(stream, 7);
    assert_next_eq!(stream, 8);
    assert_next_eq!(stream, 9);
    assert_closed!(stream);
}

#[test]
fn switch_on_channel() {
    let (tx, rx) = mpsc::unbounded_channel();

    let mut stream = UnboundedReceiverStream::new(rx).switch();
    assert_pending!(stream);

    tx.send(Either::Left(stream::iter(vec![1]))).unwrap();
    assert_next_eq!(stream, 1);
    assert_pending!(stream);

    tx.send(Either::Right(stream::pending::<i32>())).unwrap();
    assert_pending!(stream);

    tx.send(Either::Right(stream::pending())).unwrap();
    tx.send(Either::Left(stream::iter(vec![0]))).unwrap();
    assert_next_eq!(stream, 0);
    assert_pending!(stream);

    tx.send(Either::Left(stream::iter(vec![0]))).unwrap();
    tx.send(Either::Right(stream::pending())).unwrap();
    assert_pending!(stream);

    tx.send(Either::Left(stream::iter(vec![10, 20]))).unwrap();
    drop(tx);

    assert_next_eq!(stream, 10);
    assert_next_eq!(stream, 20);
    assert_closed!(stream);
}
