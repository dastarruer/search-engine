#[cfg(test)]
use std::path::PathBuf;

use scraper::{Html, Selector};

struct Term<'a> {
    term: &'a str,
    idf: i32,
    document_frequency: i32
}

impl<'a> Term<'a> {
    fn new(term: &'a str) -> Self {
        Term { term, idf: 0, document_frequency: 0 }
    }

    ///  Find the number of times that a [`Term`] appears in a given HTML document.
    ///
    /// This is called the *term frequency* of a term.
    fn get_tf_in_html(&self, document: Html) -> i32 {
        let selector = Selector::parse("body").unwrap();

        let mut count = 0;

        // TODO: Convert from loop to iterator
        for element in document.select(&selector) {
            for sentence in element.text() {
                for word in sentence.split_whitespace() {
                    if word == self.term {
                        count += 1;
                    }
                }
            }
        }

        count
    }
}

/// Return the path of a file in src/test-files given just its filename.
#[cfg(test)]
pub fn test_file_path_from_filepath(filename: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR gets the source dir of the project
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("test-files")
        .join(filename)
}

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod test {
    use std::fs;

    use scraper::Html;

    use crate::{Term, test_file_path_from_filepath};

    #[test]
    fn test_get_tf_in_html() {
        let html = fs::read_to_string(test_file_path_from_filepath("tf.html")).unwrap();
        let html = Html::parse_document(html.as_str());

        let term = Term::new("hello");

        assert_eq!(term.get_tf_in_html(html), 4);
    }
}
