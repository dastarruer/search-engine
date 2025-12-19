# Crawler

An implementation of a web-crawler, built entirely from scratch.

## Usage

A list of 'seed URLs' are required for the crawler to start. There is a starter list in the `sites.txt` file, containing a portion of the top 100 sites on the internet. However, you can add or remove sites as you wish.

To run the crawler by itself, run:

```sh
docker compose up crawler -d
```

### Benchmarks/Tests

To run all tests (integration and unit tests), run:

```sh
cargo test
```

To run all benchmarks, run:

```sh
cargo bench --features bench-utils
```

## Features

- Blocks all non-English sites
- Blocks all adult websites
- Handles `429 Too Many Requests` HTTP status codes

> [!NOTE]
> When blocking adult websites, many sites will be flagged as false positives, as the crawler takes a more conservative approach when checking for adult content.

## How it works

1. A webpage is crawled, and its HTML content is analyzed for URLs to other webpages. This content is then stored in the database for the indexer to process later.
2. The URLs to other pages are queued in the database and stored in local memory. Storing them in the database ensures that if the crawler is restarted, then queued URLs can never be lost.
3. A new URL is popped from the queue, and the process repeats.
