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
    text = r""""Hello" is a song recorded by British singer-songwriter Adele"""
    assert snippet_generator._SnippetGenerator__split_text_into_semantic_chunks(
        text
    ) == [
        '"Hello"',
        "is a song recorded by British",
        "singer-songwriter Adele",
    ]


def test_extract_text():
    html_string = r"""<body>
                        <p>hippopotamus hippopotamus hippopotamus</p>
                      </body>"""
    assert extract_text(html_string) == "hippopotamus hippopotamus hippopotamus"
