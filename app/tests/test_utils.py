import pytest
from utils import extract_text, _SnippetGenerator


@pytest.fixture
def snippet_generator() -> _SnippetGenerator:
    return _SnippetGenerator()


def test_extract_text():
    html_string = r"""<body>
                        <p>hippopotamus hippopotamus hippopotamus</p>
                      </body>"""
    assert extract_text(html_string) == "hippopotamus hippopotamus hippopotamus"


def test_split_text_by_punctuation(snippet_generator: _SnippetGenerator):
    text = "hello. hello, hello? hello!"
    assert snippet_generator._SnippetGenerator__split_text_by_punctuation(text) == [
        "hello.",
        "hello,",
        "hello?",
        "hello!",
    ]
