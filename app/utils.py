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
        """
        Returns an HTML snippet of visible text in an HTML string based on a list of query terms.

        The function extracts visible text, which is any text that is displayed to the user. This excludes tags like <script>, <style>, alt text on images, etc.

        This text is then split by punctuation into multiple phrases, and finds the first phrase that contains at least one term from the query. This phrase is the root phrase of the snippet, and will be wrapped in a <span> tag to add styling. The final snippet will be truncated with a '...' if it exceeds a certain character count.`

        An empty string will be returned if no phrases with terms from the query are found.
        """
        text = extract_text(html_string)
        pattern = self.__compile_regex_for_query(query)
        phrases = self.__split_text_by_punctuation(text)

        snippet = ""
        for i, phrase in enumerate(phrases):
            # If a term in the query is found in the phrase
            if pattern.search(phrase):
                (phrase, phrases) = self.__elongate_phrase(i, phrases, phrase)

                # Convert any html tags to plain-text
                phrase = escape(phrase.lstrip())

                # Bolden the phrase with the term from the query
                snippet += rf'<span class="prompt-bold">{phrase}</span>'

                snippet = self.__elongate_snippet(i, phrases, snippet)

                SNIPPET_WIDTH_CHARS = 200
                snippet = shorten(snippet, width=SNIPPET_WIDTH_CHARS, placeholder="...")

                return snippet

        return snippet

    def __split_text_by_punctuation(self, text: str) -> list[str]:
        return re.findall(r"[^?.,!]+[?.,!]?|[^?.,!]+$", text)

    def __compile_regex_for_query(self, query):
        return re.compile(
            r"(" + "|".join(map(re.escape, query)) + r")[^\w\s]*", re.IGNORECASE
        )

    def __elongate_snippet(
        self, current_index: int, phrases: list[str], snippet: str
    ) -> str:
        counter = 1
        while len(snippet) < 200 or counter < 3:
            # Add second phrase to snippet
            if current_index + counter < len(phrases):
                snippet += " " + phrases[current_index + counter]
                counter += 1
            # Add the phrase before the current one if there is no phrase afterwards
            else:
                snippet = phrases[current_index - 1] + " " + snippet
                return snippet
        return snippet

    def __elongate_phrase(
        self, current_index: int, phrases: list[str], current_phrase: str
    ) -> tuple[str, list[str]]:
        # Keep track of which phrases are used so we can remove them later
        _last_used_index = len(phrases) - 1

        counter = 1
        while len(current_phrase) < 60 and current_index + counter < len(phrases):
            current_phrase += " " + phrases[current_index + counter]
            _last_used_index = current_index + counter
            counter += 1

        return (current_phrase, phrases)


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
