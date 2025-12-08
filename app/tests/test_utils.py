import pytest
from utils import extract_text, _SnippetGenerator
from pathlib import Path


@pytest.fixture
def snippet_generator() -> _SnippetGenerator:
    return _SnippetGenerator()


def test_generate_snippet(snippet_generator: _SnippetGenerator):
    fixture_path = Path(__file__).parent / "fixtures" / "wikipedia_article.html"
    with open(fixture_path, "r") as f:
        html_string = f.read()
    query = ["hello"]
    expected_snippet = r"""<span class="prompt-bold">&#34;Hello&#34; is a song recorded by British singer-songwriter Adele,</span> released on 23 October 2015 by XL Recordings as the lead single from her third studi..."""

    assert snippet_generator.generate_snippet(html_string, query) == expected_snippet


def test_split_text_by_punctuation(snippet_generator: _SnippetGenerator):
    text = "hello. hello hello! hello?"
    assert snippet_generator._SnippetGenerator__split_text_by_punctuation(text) == [
        "hello.",
        " hello hello!",
        " hello?",
    ]


def test_extract_text():
    html_string = r"""<body>
                        <p>hippopotamus hippopotamus hippopotamus</p>
                      </body>"""
    assert extract_text(html_string) == "hippopotamus hippopotamus hippopotamus"
