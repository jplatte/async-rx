# async-rx

Utility functions for async reactive programming.

This crate is intentionally very small as it only provides utilities that are not already found in futures-util. It is meant as a supplement, not a replacement for the existing well-known futures crates.

Currently provided functionality:

- `StreamExt::dedup` for deduplicating consecutive equal items
- `StreamExt::dedup_by_key` for deduplicating consecutive items with an equal property
- `StreamExt::batch_with` for flexible batching of the stream's items
- `StreamExt::switch` for switching a stream of streams
