# App

A website to serve search results to users built with Flask.

## Features

- Displays 10 search results to the user based on their query
- Filters out stop words from the user's query for more relevant results
- Displays results in the same format as other, modern search engines:
  - Displays the [title of the result](../assets/result-anatomy/title.png) as a clickable link.
  - Displays the [domain of the result](../assets/result-anatomy/domain.png).
  - Displays a [breadcrumb of the URL route](../assets/result-anatomy/breadcrumb.png).
  - Displays a [summary of each result](../assets/result-anatomy/summary.png) based on the user's query.

## How it works

1. The user types a query into the search box.
2. All stop words are removed, and the query is split into separate terms.
3. The database is queried for the most relevant pages, and the results are shown to the user.
