use std::error::Error;

use async_rx::StreamExt as _;
use futures_util::stream;
use stream_assert::{assert_closed, assert_next_eq, assert_pending};
use tokio::sync::mpsc::{channel, unbounded_channel};
use tokio_stream::wrappers::{ReceiverStream, UnboundedReceiverStream};

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
async fn empty_primary_stream() {
    let (_drainer, drainer_receiver) = channel::<()>(1);

    let (stream_sender, stream_receiver) = unbounded_channel::<usize>();
    let mut batch_stream = UnboundedReceiverStream::new(stream_receiver)
        .batch_with(ReceiverStream::new(drainer_receiver));

    // Let's stop the batch stream by closing the stream sender.
    drop(stream_sender);

    // Stopping the batch stream forces it to drain its items. There was no item in
    // it, so it's closed immediately.
    assert_closed!(batch_stream);
}

#[tokio::test]
async fn trigger_happy_batch_stream() -> Result<(), Box<dyn Error>> {
    let (drainer, drainer_receiver) = unbounded_channel::<()>();

    let mut stream =
        stream::pending::<u8>().batch_with(UnboundedReceiverStream::new(drainer_receiver));

    // The primary stream is always pending, the combined stream should be too.
    assert_pending!(stream);

    // Even if the drainer stream produces items.
    drainer.send(())?;
    assert_pending!(stream);

    // However many!
    drainer.send(())?;
    drainer.send(())?;
    drainer.send(())?;

    assert_pending!(stream);
    assert_pending!(stream);
    assert_pending!(stream);
    assert_pending!(stream);

    Ok(())
}
