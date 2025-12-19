# TF-IDF Indexer

An implementation of a [TF-IDF](https://www.geeksforgeeks.org/machine-learning/understanding-tf-idf-term-frequency-inverse-document-frequency/) indexer.

## Tests

To run all tests (integration and unit tests), run:

```sh
cargo test
```

## How it works

1. Crawled webpages are fetched in batches, and stored in a queue in memory.
2. A page is popped from the queue, and is parsed for terms (excluding [stop terms](https://en.wikipedia.org/wiki/Stop_word))
3. Once the queue is empty, all terms in memory are pushed to the database, and the process repeats.
