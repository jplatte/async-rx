# 0.1.3

- Fix `BatchWith` yielding an empty `Vec` when the secondary stream yields an
  item without the primary stream having yielded any

# 0.1.2

- Add `StreamExt::switch` for dynamically switching over from one stream to the
  next as soon as it's ready

# 0.1.1

- Add `StreamExt::batch_with` for flexible batching of the stream's items

# 0.1.0

Initial release.
