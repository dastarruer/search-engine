# P.R.A.K.S.

A general-purpose search engine, similar to Google or DuckDuckGo.

## Demo

https://github.com/user-attachments/assets/631c23b8-5a65-4eb5-981c-214309882228

### Screenshots

<!-- This table was 'stolen' from here: https://github.com/poogas/Ax-Shell/blob/5610a831c36493036a1a10fcb9547d8cda204005/README.md?plain=1#L32 -->
<table align="center">
  <tr>
    <td colspan="4"><img src="assets/example-results/main_page.png"></td>
  </tr>
  <tr>
    <td colspan="1"><img alt="Screenshot of main page." src="assets/example-results/gumball.png"></td>
    <td colspan="1"><img alt="Screenshot of 'gumball' search results" src="assets/example-results/persona_5_strikers.png"></td>
    <td colspan="1" align="center"><img alt="Screenshot of 'persona 5 strikers' search results" src="assets/example-results/hitman.png"></td>
  </tr>
</table>

## Features

- A [web crawler](./services/crawler/README.md) built from scratch
- An [indexer](./services/tf-idf-indexer/README.md) using the [TF-IDF](https://www.geeksforgeeks.org/machine-learning/understanding-tf-idf-term-frequency-inverse-document-frequency/) algorithm, also built from scratch
- A [website](./app/README.md) with an interface similar to other search engines, such as Google or DuckDuckGo
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
