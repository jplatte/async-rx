use std::{pin::pin, task::Poll};

use async_rx::StreamExt as _;
use futures_util::{future::Either, stream, StreamExt as _};
use stream_assert::{assert_closed, assert_next_eq, assert_pending};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

#[test]
fn switch_empty() {
    let mut stream = stream::empty::<stream::Empty<stream::Empty<()>>>().switch();
    assert_closed!(stream);
}

#[test]
fn switch_eagerly_polls_outer_stream() {
    let mut stream =
        stream::iter([stream::iter([1, 2, 3]), stream::iter([4, 5, 6])]).fuse().switch();
    assert_next_eq!(stream, 4);
    assert_next_eq!(stream, 5);
    assert_next_eq!(stream, 6);
    assert_closed!(stream);
}

#[test]
fn switch_outer_stream_is_closed_then_stream_is_closed() {
    let mut stream = stream::poll_fn(|_| Poll::Ready(None::<stream::Empty<()>>)).fuse().switch();
    assert_closed!(stream);
}

#[test]
fn switch_outer_stream_is_pending_then_stream_is_pending() {
    let mut stream =
        stream::poll_fn(|_| Poll::<Option<stream::Empty<()>>>::Pending).fuse().switch();
    assert_pending!(stream);
}

#[test]
fn switch_outer_stream_is_closed_after_first_poll_and_inner_stream_is_closed_then_stream_is_closed()
{
    let mut stream =
        pin!(stream::once(async { stream::poll_fn(|_| Poll::Ready(None::<()>)) }).switch());
    assert_closed!(stream);
}

#[test]
fn switch_inner_stream_is_closed_then_stream_is_pending() {
    let mut stream = {
        let mut yielded_once = false;

        stream::poll_fn(move |_| {
            if yielded_once {
                Poll::Pending
            } else {
                yielded_once = true;

                Poll::Ready(Some(stream::poll_fn(|_| Poll::Ready(None::<()>))))
            }
        })
        .fuse()
        .switch()
    };
    assert_pending!(stream);
}

#[test]
fn switch_inner_stream_is_pending_then_stream_is_pending() {
    let mut stream = {
        let mut yielded_once = false;

        stream::poll_fn(move |_| {
            if yielded_once {
                Poll::Pending
            } else {
                yielded_once = true;

                Poll::Ready(Some(stream::pending::<()>()))
            }
        })
        .fuse()
        .switch()
    };
    assert_pending!(stream);
}

#[test]
fn switch_on_channel() {
    let (tx, rx) = mpsc::unbounded_channel();

    let mut stream = UnboundedReceiverStream::new(rx).fuse().switch();
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
