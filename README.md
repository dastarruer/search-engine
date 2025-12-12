# P.R.A.K.S.

A general-purpose search engine, similar to Google or DuckDuckGo.

## Features

- A synchronous web-crawler built from scratch
- An indexer using the [TF-IDF](https://www.geeksforgeeks.org/machine-learning/understanding-tf-idf-term-frequency-inverse-document-frequency/) algorithm, also built from scratch
- A website with an interface similar to other search engines, such as Google or DuckDuckGo
- An extensive test suite of 50+ unit/integration tests for all services

## Usage

First, clone the repo:

```sh
git clone https://github.com/dastarruer/search-engine/
cd search-engine
```

Then, create a `.env` file in the root of `search-engine/` with the following environment variables:

```
# Change these to be whatever you like
DB_PORT=5432
DB_NAME=postgres
DB_USER=postgres
DB_ENDPOINT=db
DB_PASSWORD=123
```

Finally, run the following command to start all services at once (crawler, indexer, and website):

```sh
docker compose up -d
```

To use the website, go to http://localhost:80 in your web browser of choice.

> [!NOTE]
> Please note that there will not be many search results at first. But as the crawler and indexer continue to gather more and more results, both the accuracy and number of search results will increase.
