use std::collections::HashSet;

use once_cell::sync::Lazy;
use ordered_float::OrderedFloat;
use scraper::{Html, Selector};

static BODY_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("body").unwrap());

static STOP_WORDS: Lazy<HashSet<Term>> = Lazy::new(|| {
    stop_words::get(stop_words::LANGUAGE::English)
        .iter()
        .copied()
        .map(|t| Term::new(t.to_string()))
        .collect()
});

mod helper {
    #[allow(non_camel_case_types)]
    pub type f32_helper = f32;
}
#[allow(non_camel_case_types)]
// This float type allows us to implement `Hash` for `Term`, so we can put it in a `HashSet`
type ordered_f32 = ordered_float::OrderedFloat<helper::f32_helper>;

#[derive(PartialEq, Eq, Hash, Debug)]
struct Term {
    pub term: String,

    /// The inverse document frequency of a term.
    ///
    /// This measures how rare a term is across documents. If the term appears in many documents, then the IDF is low. If the term only appears in one or two documents, the IDF is high.
    idf: ordered_f32,

    /// The amount of documents that contain this term. Used for calculating [`Term::idf`].
    document_frequency: i32,
}

impl Term {
    fn new(term: String) -> Self {
        Term {
            term,
            idf: ordered_float::OrderedFloat(0.0),
            document_frequency: 0,
        }
    }

    /// Find the number of times that a [`Term`] appears in a given piece of text.
    ///
    /// This is called the *term frequency* of a term.
    fn get_tf<'b>(&self, text: &Vec<Term>) -> i32 {
        text.iter()
            .filter(|t| t.term.eq_ignore_ascii_case(&self.term))
            .count() as i32
    }

    fn update_idf(&mut self, num_documents: i32) {
        let idf = num_documents as f32 / self.document_frequency as f32;
        self.idf = OrderedFloat(idf.log10());
    }

    /// Checks if the `Term` is a stop word.
    ///
    /// A stop word is a common word such as 'is,' 'was,' 'has,' etc.
    /// These words are not necessary to index, since they carry little semantic meaning. These can therefore be filtered
    /// out.
    fn is_stop_word(&self) -> bool {
        STOP_WORDS.contains(&self)
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

trait ExtractTerms {
    fn extract_relevant_terms(&self) -> Vec<Term>;
}

impl ExtractTerms for Html {
    /// Extract relevant [`Term`]s from [`Html`].
    ///
    /// First filters out common 'stop words' (see [`Term::is_stop_word`] for more information), and then returns the resulting list of [`Term`]s.
    // TODO: Strip punctuation
    fn extract_relevant_terms(&self) -> Vec<Term> {
        self.select(&BODY_SELECTOR)
            .flat_map(|e| e.text())
            .flat_map(|t| t.split_whitespace())
            .map(|t| t.trim().chars().filter(|c| c.is_alphanumeric()).collect())
            .map(|t: String| Term::new(t))
            .filter(|t| !t.is_stop_word())
            .collect()
    }
}

#[cfg(test)]
mod test {
    use std::fs;

    use scraper::Html;

    use crate::{ExtractTerms, Term, test_file_path_from_filepath};

    #[test]
    fn test_get_tf_of_term() {
        let html = fs::read_to_string(test_file_path_from_filepath("tf.html")).unwrap();
        let html = Html::parse_document(html.as_str());

        let term = Term::new(String::from("hippopotamus"));

        assert_eq!(term.get_tf(&html.extract_relevant_terms()), 4);
    }

    #[test]
    fn test_extract_terms() {
        let html = Html::parse_document(
            r#"
            <body>
                <p>hippopotamus hippopotamus hippopotamus</p>
            </body>"#,
        );
        let expected_terms = vec![
            Term::new(String::from("hippopotamus")),
            Term::new(String::from("hippopotamus")),
            Term::new(String::from("hippopotamus")),
        ];

        assert_eq!(html.extract_relevant_terms(), expected_terms);
    }

    #[test]
    fn test_update_idf() {
        let mut term = Term::new(String::from("hippopotamus"));
        term.document_frequency = 2;

        term.update_idf(2);

        assert_eq!(term.idf, 0.0);
    }

    #[test]
    fn test_filter_stop_words() {
        let html =
            fs::read_to_string(test_file_path_from_filepath("filter_stop_words.html")).unwrap();
        let html = Html::parse_document(html.as_str());

        let terms = html.extract_relevant_terms();

        let included_terms = vec![
            Term::new(String::from("hippopotamus")),
            Term::new(String::from("ladder")),
        ];

        assert_eq!(terms, included_terms);
    }
}
