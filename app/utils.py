import os
import re
from textwrap import shorten

import psycopg2
from lxml import html
from markupsafe import escape
from psycopg2.extensions import cursor


def extract_text(html_string: str) -> str:
    tree = html.fromstring(html_string)
    paragraphs = tree.xpath("//p")
    # TODO: Replace <br> tags with spaces
    return " ".join(p.text_content() for p in paragraphs)


class _SnippetGenerator:
    def generate_snippet(self, html_string: str, query: list[str]) -> str:
        text = extract_text(html_string)
        pattern = self.__compile_regex_for_query(query)
        phrases = self.__split_text_by_punctuation(text)

        snippet = ""
        for i, phrase in enumerate(phrases):
            # If a term in the query is found in the phrase
            if pattern.search(phrase):
                # Convert any html tags to plain-text
                phrase = escape(phrase)

                # Bolden the phrase with the term from the query
                snippet += rf'<span class="prompt-bold">{phrase}</span>'

                snippet = self.__elongate_phrase(i, phrases, snippet, phrase)
                break

        SNIPPET_WIDTH_CHARS = 270
        snippet = shorten(snippet, width=SNIPPET_WIDTH_CHARS, placeholder="...")

        if snippet and snippet[-1] not in ".!?":
            snippet = snippet[:-1] + "..."

        return snippet

    def __split_text_by_punctuation(self, text):
        return re.findall(r"[^?.,!]+[?.,!]?|[^?.,!]+$", text)

    def __compile_regex_for_query(self, query):
        return re.compile(
            r"(" + "|".join(map(re.escape, query)) + r")[^\w\s]*", re.IGNORECASE
        )

    def __elongate_phrase(
        self, current_index: int, phrases: list[str], snippet: str, current_phrase: str
    ) -> str:
        # Add second phrase to snippet
        if current_index + 1 < len(phrases):
            snippet += phrases[current_index + 1]
        # Add the phrase before the current one if there is no phrase afterwards
        else:
            snippet = phrases[current_index - 1] + snippet
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
