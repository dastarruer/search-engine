import os

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
        SELECT pages.url, SUM(tf_idf_score::real) AS total_score
        FROM terms
        -- Basically, each() will extract each value in an hstore into an array. Then,
        -- CROSS JOIN LATERAL will then extract the array elements into separate rows,
        -- with columns called `page_id` and `tf_idf_score`.
        CROSS JOIN LATERAL each(tf_idf_scores) AS kv(page_id, tf_idf_score)
        -- Then, connect the two tables by their page id columns
        JOIN pages ON pages.id = page_id::integer
        WHERE term = ANY(%s)
        -- If two terms have TF-IDF scores for the same page, then add them up
        GROUP BY pages.id
        -- Order with tf_idf scores from largest to smallest
        ORDER BY total_score DESC;
    """

    conn.execute(sql, (query,))
    results = conn.fetchall()

    return str(results)


if __name__ == "__main__":
    app.run()


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
