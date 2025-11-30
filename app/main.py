import os
import tldextract
from urllib.parse import urlparse
from lxml import html
import re
from textwrap import shorten

import psycopg2
from flask import Flask, render_template, request
from psycopg2.extensions import cursor

app = Flask(__name__)


@app.route("/")
def hello_world():
    return render_template("index.html")


@app.route("/search")
def search_results():
    query = request.args.get("q").split()
    conn = db_conn()

    sql = """
        SELECT pages.url, SUM(tf_idf_score::real) AS total_score, pages.title, pages.html
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
        -- Order with tf_idf scores from largest to smallest
        ORDER BY total_score DESC
        LIMIT 10;
    """

    conn.execute(sql, (query,))
    results = conn.fetchall()

    if not results:
        return render_template("no_results.html")

    # Add the page domain and breadcrumb to the results, so it can be shown to the user on the frontend
    for i, result in enumerate(results):
        url = result[0]
        html_string = result[3]

        domain = tldextract.extract(url).domain.title()
        breadcrumb = generate_breadcrumb(url)
        snippet = get_snippet(html_string, query)

        result = list(result)
        result[3] = snippet
        result.append(domain)
        result.append(breadcrumb)

        results[i] = tuple(result)

    # return str(results)
    return render_template("results.html", results=results)


if __name__ == "__main__":
    app.run()


def generate_breadcrumb(url: str) -> str:
    url = urlparse(url)

    breadcrumb = url.netloc + url.path
    breadcrumb = breadcrumb.replace("/", " > ")
    breadcrumb = breadcrumb.removesuffix(" > ")  # Some paths may have a trailing `/`

    return breadcrumb


def extract_text(html_string: str) -> str:
    tree = html.fromstring(html_string)
    paragraphs = tree.xpath("//p")
    # TODO: Replace <br> tags with spaces
    return " ".join(p.text_content() for p in paragraphs)


def get_snippet(html_string: str, query: list[str]) -> str:
    # text = shorten(extract_text(html_string), width=270, placeholder="...")
    text = extract_text(html_string)

    # Remove the character before the placeholder if it is punctuation
    if len(text) >= 4 and not text[-4].isalnum():
        text = text[:-4] + text[-3:]

    # Create a regex to match query terms
    pattern = re.compile(
        r"(" + "|".join(map(re.escape, query)) + r")[^\w\s]*", re.IGNORECASE
    )

    # Split text by punctuation
    phrases = re.findall(r"[^?.,!]+[?.,!]?|[^?.,!]+$", text)

    snippet = ""
    for i, phrase in enumerate(phrases):
        if pattern.search(phrase):
            snippet += rf'<span class="prompt-bold">{phrase}</span>'

            counter = 1
            # If the snippet is too small, then add more phrases
            while len(phrase) < 50:
                if i + counter < len(phrases):
                    snippet = snippet + phrases[i + counter]
                elif i + counter >= len(phrases):
                    snippet = phrases[i - 1] + snippet
                    break
                counter += 1

            # very janky, but this will always add a second phrase to the snippet even if the length is too small
            # TODO: Move this into while loop
            if i + 1 < len(phrases):
                snippet += phrases[i + 1]
            # Add the phrase before the current one if there is no phrase afterwards
            else:
                new_snippet = phrases[i - 1] + snippet
                snippet = new_snippet
            break

    snippet = shorten(snippet, width=270, placeholder="...")

    if snippet[-1] != "." or snippet[-1] != "?" or snippet[-1] != "!":
        snippet = snippet[:-1]
        snippet += "..."

    return snippet


def retrieve_env_var(var: str) -> str:
    try:
        return os.environ[var]
    except KeyError:
        raise RuntimeError(f"Missing required environment variable: {var}")


def db_conn() -> cursor:
    database = retrieve_env_var("DB_NAME")
    user = retrieve_env_var("DB_USER")
    password = retrieve_env_var("DB_PASSWORD")
    host = retrieve_env_var("DB_ENDPOINT")

    conn = psycopg2.connect(
        database=database, user=user, password=password, host=host, port="5432"
    )

    cursor = conn.cursor()

    return cursor
