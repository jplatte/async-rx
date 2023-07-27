use std::error::Error;

use async_rx::StreamExt as _;
use futures_util::{stream, FutureExt, StreamExt};
use stream_assert::{assert_closed, assert_next_eq, assert_pending};
use tokio::sync::mpsc::{channel, unbounded_channel};
use tokio_stream::wrappers::{ReceiverStream, UnboundedReceiverStream};

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

#[tokio::test]
async fn batch_with() -> Result<(), Box<dyn Error>> {
    // The material we need to drain the batch stream.
    let (drainer, drainer_receiver) = channel::<()>(1);

    // The material for the batch stream.
    let (stream_sender, stream_receiver) = unbounded_channel();
    let mut batch_stream = UnboundedReceiverStream::new(stream_receiver)
        .batch_with(ReceiverStream::new(drainer_receiver));

    // The batch stream is empty, and is pending.
    assert_pending!(batch_stream);

    // Send new data onto the batch stream.
    stream_sender.send(1)?;
    stream_sender.send(2)?;
    stream_sender.send(3)?;

    // The batch stream is not empty, but it's still pending.
    assert_pending!(batch_stream);

    // Let's drain the batch stream.
    drainer.send(()).await?;

    // Oh, a batch.
    assert_next_eq!(batch_stream, vec![1, 2, 3]);

    // Send new data onto the batch stream.
    stream_sender.send(4)?;
    stream_sender.send(5)?;

    // And it's still pending.
    assert_pending!(batch_stream);

    // Let's drain it again.
    drainer.send(()).await?;

    // Oh, another batch.
    assert_next_eq!(batch_stream, vec![4, 5]);
    assert_pending!(batch_stream);

    // Send new data onto the batch stream.
    stream_sender.send(6)?;
    stream_sender.send(7)?;
    stream_sender.send(8)?;

    // The batch stream is not empty, and is pending.
    assert_pending!(batch_stream);

    // Let's stop the batch stream by closing the stream sender.
    drop(stream_sender);

    // Stopping the batch stream forces it to drain its items.
    assert_next_eq!(batch_stream, vec![6, 7, 8]);
    assert_closed!(batch_stream);

    Ok(())
}

#[tokio::test]
async fn empty_batch_with() -> Result<(), Box<dyn Error>> {
    let (_drainer, drainer_receiver) = channel::<()>(1);

    let (stream_sender, stream_receiver) = unbounded_channel::<usize>();
    let mut batch_stream = UnboundedReceiverStream::new(stream_receiver)
        .batch_with(ReceiverStream::new(drainer_receiver));

    // Let's stop the batch stream by closing the stream sender.
    drop(stream_sender);

    // Stopping the batch stream forces it to drain its items. There was no item in
    // it, so it's closed immediately.
    assert_closed!(batch_stream);

    Ok(())
}
