from utils import extract_text


def test_extract_text():
    html_string = r"""<body>
                        <p>hippopotamus hippopotamus hippopotamus</p>
                      </body>"""
    assert extract_text(html_string) == "hippopotamus hippopotamus hippopotamus"
