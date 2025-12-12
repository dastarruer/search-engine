from textwrap import shorten
from urllib.parse import urlparse

import nltk
import tldextract
from flask import Flask, render_template, request
from nltk.corpus import stopwords
from nltk.tokenize import word_tokenize
from utils import _SnippetGenerator, db_conn

app = Flask(__name__)

nltk.download("stopwords")
nltk.download("punkt_tab")
stop_words = set(stopwords.words("english"))


class SearchResult:
    def __init__(self, url="", title=""):
        self.url = url
        self.title = title
        self.snippet_generator = _SnippetGenerator()

        if url:
            self.domain = self.__get_domain(url)
            self.breadcrumb = self.__generate_breadcrumb(url)

    def set_snippet(self, html_string, query) -> None:
        self.snippet = self.snippet_generator.generate_snippet(html_string, query)

    def __get_domain(self, url: str) -> str:
        return tldextract.extract(url).domain.title()

    def __generate_breadcrumb(self, url: str) -> str:
        url = urlparse(url)

        breadcrumb = url.netloc + url.path
        breadcrumb = breadcrumb.replace("/", " > ")
        breadcrumb = breadcrumb.removesuffix(
            " > "
        )  # Some paths may have a trailing `/`

        return breadcrumb


@app.route("/")
def front_page() -> str:
    return render_template("index.html")


@app.route("/search")
def search_results() -> str:
    query = word_tokenize(request.args.get("q").lower())
    query = [term for term in query if term not in stop_words]

    conn = db_conn()

    sql = """
        SELECT pages.url, pages.title, pages.html
        FROM terms
        -- Basically, each() will extract each value in an hstore into an array. Then,
        -- CROSS JOIN LATERAL will then extract the array elements into separate rows,
        -- with columns called `page_id` and `tf_idf_score`.
        CROSS JOIN LATERAL each(tf_idf_scores) AS kv(page_id, tf_idf_score)
        -- Then, connect the two tables by their page id columns
        JOIN pages ON pages.id = page_id::integer
        WHERE term = ANY(%s)
        -- If two terms have TF-IDF scores for the same page, then add them up
        GROUP BY pages.id, pages.url, pages.title, pages.html
        -- Order with tf_idf scores from largest to smallest, giving a boost to pages with more terms in the query
        ORDER BY SUM(tf_idf_score::real) * COUNT(term) DESC
        LIMIT 10;
    """

    conn.execute(sql, (query,))
    results = conn.fetchall()

    if not results:
        return render_template("no_results.html")

    # Add the page domain and breadcrumb to the results, so it can be shown to the user on the frontend
    for i, result in enumerate(results):
        url = result[0]
        title = result[1]
        html_string = result[2]

        TITLE_WIDTH_CHARS = 60
        result = SearchResult(
            url=url,
            title=shorten(title, width=TITLE_WIDTH_CHARS, placeholder="..."),
        )
        result.set_snippet(html_string, query)

        results[i] = result

    return render_template("results.html", results=results)


if __name__ == "__main__":
    app.run()
