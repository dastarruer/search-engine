use scraper::{Html, Selector};

struct Term<'a> {
    pub term: &'a str,

    /// The inverse document frequency of a term.
    ///
    /// This measures how rare a term is across documents. If the term appears in many documents, then the IDF is low. If the term only appears in one or two documents, the IDF is high.
    idf: f32,

    /// The amount of documents that contain this term. Used for calculating [`Term::idf`].
    document_frequency: i32,
}

impl<'a> Term<'a> {
    fn new(term: &'a str) -> Self {
        Term {
            term,
            idf: 0.0,
            document_frequency: 0,
        }
    }

    /// Find the number of times that a [`Term`] appears in a given HTML document.
    ///
    /// This is called the *term frequency* of a term.
    fn get_tf_in_html(&self, document: Html) -> i32 {
        let selector = Selector::parse("body").unwrap();

        document
            .select(&selector)
            .flat_map(|e| e.text()) // flatten text nodes
            .flat_map(|t| t.split_whitespace()) // flatten words
            .filter(|word| word == &self.term)
            .count() as i32
    }

    fn update_idf(&mut self, num_documents: i32) {
        self.idf = f32::log10((num_documents / self.document_frequency) as f32);
    }
}

/// Return the path of a file in src/test-files given just its filename.
#[cfg(test)]
pub fn test_file_path_from_filepath(filename: &str) -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR gets the source dir of the project
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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

    #[test]
    fn test_update_idf() {
        let mut term = Term::new("hello");
        term.document_frequency = 2;

        term.update_idf(2);

        assert_eq!(term.idf, 0.0);
    }
}
