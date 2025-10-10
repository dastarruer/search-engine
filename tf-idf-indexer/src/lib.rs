use std::collections::{HashMap, HashSet};

use once_cell::sync::Lazy;
use ordered_float::OrderedFloat;
use scraper::{Html, Selector};

static BODY_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("body").unwrap());

static STOP_WORDS: Lazy<HashSet<StopWordTerm>> = Lazy::new(|| {
    stop_words::get(stop_words::LANGUAGE::English)
        .iter()
        .copied()
        .map(StopWordTerm::new)
        .collect()
});

mod helper {
    #[allow(non_camel_case_types)]
    pub type f32_helper = f32;
}
#[allow(non_camel_case_types)]
// This float type allows us to implement `Hash` and `Eq` for `Term`, so we can put it in a `HashSet`
type ordered_f32 = ordered_float::OrderedFloat<helper::f32_helper>;

#[derive(PartialEq, Eq, Debug, Clone)]
struct Term {
    pub term: String,

    /// The inverse document frequency of a term.
    ///
    /// This measures how rare a term is across documents. If the term appears in many documents, then the IDF is low. If the term only appears in one or two documents, the IDF is high.
    idf: ordered_f32,

    /// The amount of documents that contain this term. Used for calculating [`Term::idf`].
    document_frequency: i32,

    /// The TF scores of each [`Document`].
    ///
    /// TF is measured as the term frequency of a [`Term`], or how many times a term appears in a given [`Document`].
    tf_scores: HashMap<Document, ordered_f32>,

    /// The TF-IDF scores of each [`Document`].
    ///
    /// TF-IDF is measured as the term frequency of a [`Term`] in a [`Document`] multiplied by [`Term::idf`].
    tf_idf_scores: HashMap<Document, ordered_f32>,
}

// Manually implement the Hash trait since HashMap does not implement Hash
impl std::hash::Hash for Term {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Just hash the term instead of anything else
        self.term.to_lowercase().hash(state);
    }
}

impl Term {
    fn new(term: String) -> Self {
        Term {
            term,
            idf: ordered_float::OrderedFloat(0.0),
            document_frequency: 0,
            tf_scores: HashMap::new(),
            tf_idf_scores: HashMap::new(),
        }
    }

    /// Find the number of times that a [`Term`] appears in a given piece of text.
    ///
    /// This is called the *term frequency* of a term. This is useful when
    /// calculating the TF-IDF score of a term, which is used to check how
    /// frequent a [`Term`] is in one document, and how rare it is in other
    /// documents.
    fn get_tf<'b>(&self, terms: &Vec<Term>) -> i32 {
        terms
            .iter()
            .filter(|t| t.term.eq_ignore_ascii_case(&self.term))
            .count() as i32
    }

    /// Update the IDF score of a [`Term`] (see [`Term::idf`] for more details).
    ///
    /// This is useful when calculating the TF-IDF score of a term, which is
    /// used to check how frequent a [`Term`] is in one document, and how rare
    /// it is in other documents.
    fn update_total_idf(&mut self, num_documents: i32) {
        let idf = num_documents as f32 / self.document_frequency as f32;
        self.idf = OrderedFloat(idf.log10());
    }

    /// Checks if the `Term` is a stop word.
    ///
    /// A stop word is a common word such as 'is,' 'was,' 'has,' etc.
    /// These words are not necessary to index, since they carry little semantic meaning. These can therefore be filtered
    /// out.
    fn is_stop_word(&self) -> bool {
        STOP_WORDS.contains(&StopWordTerm::new(&self.term))
    }
}

#[derive(PartialEq, Eq, Debug, Hash)]
/// A simpler verson of [`Term`] just for storing stop words.
///
/// A stop word is a common word such as 'is,' 'was,' 'has,' etc.
/// See [`Term::is_stop_word`] for more information.
struct StopWordTerm<'a> {
    pub term: &'a str,
}

impl<'a> StopWordTerm<'a> {
    fn new(term: &'a str) -> Self {
        StopWordTerm { term }
    }
}

struct Indexer {
    terms: HashMap<String, Term>,
    documents: Vec<Document>,
    num_documents: i32,
}

impl Indexer {
    fn new(starting_terms: HashMap<String, Term>) -> Self {
        Indexer {
            terms: starting_terms,
            documents: Vec::new(),
            num_documents: 0,
        }
    }

    fn parse_document(&mut self, document: Document) {
        let relevant_terms = document.extract_relevant_terms();

        self.add_document(document.clone());

        for term in relevant_terms.clone() {
            self.add_term(term);
        }

        for (_, term) in self.terms.iter_mut() {
            let tf = ordered_float::OrderedFloat(term.get_tf(&relevant_terms) as f32);

            if tf > ordered_float::OrderedFloat(0.0) {
                term.document_frequency += 1;
            }

            term.update_total_idf(self.num_documents);

            let tf_idf = tf * term.idf;

            term.tf_scores.insert(document.clone(), tf);
            term.tf_idf_scores.insert(document.clone(), tf_idf);

            // Go back and update the tf_idf scores for every other single document
            let mut tf_scores_clone = term.tf_scores.clone();
            for (document, tf) in tf_scores_clone.iter_mut() {
                let new_tf_idf = tf.clone() * tf_idf;
                term.tf_idf_scores.insert(document.clone(), new_tf_idf);
            }

            term.tf_scores = tf_scores_clone;
        }
    }

    /// Add a new [`Document`] to the list of existing documents, and increment [`Indexer::num_documents`].
    fn add_document(&mut self, document: Document) {
        self.documents.push(document);
        self.num_documents += 1;
    }

    fn add_term(&mut self, term: Term) {
        if !self.terms.contains_key(&term.term) {
            let mut new_term = term.clone();

            // Initialize tf and tf_idf for all existing documents
            for doc in &self.documents {
                new_term.tf_scores.insert(doc.clone(), OrderedFloat(0.0));
                new_term
                    .tf_idf_scores
                    .insert(doc.clone(), OrderedFloat(0.0));
            }

            self.terms.insert(new_term.term.clone(), new_term);
        }
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

#[derive(Eq, Debug, Clone)]
struct Document {
    id: i32,
    html: Html,
}

impl PartialEq for Document {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

// Manually implement the Hash trait since Html does not implement Hash
impl std::hash::Hash for Document {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Just hash the term instead of anything else
        self.id.hash(state);
    }
}

impl Document {
    fn new(html: Html, id: i32) -> Self {
        Document { html, id }
    }

    /// Extract relevant [`Term`]s from [`Html`].
    ///
    /// First filters out common 'stop words' (see [`Term::is_stop_word`] for more information), and then returns the resulting list of [`Term`]s.
    // TODO: Strip punctuation
    fn extract_relevant_terms(&self) -> Vec<Term> {
        self.html
            .select(&BODY_SELECTOR)
            .flat_map(|e| e.text())
            .flat_map(|t| t.split_whitespace())
            .map(|t| {
                t.trim()
                    .to_lowercase()
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect()
            })
            .map(|t: String| Term::new(t))
            .filter(|t| !t.is_stop_word())
            .collect()
    }
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, f32, fs};

    use scraper::Html;

    use crate::{Document, Indexer, Term, test_file_path_from_filepath};

    const DEFAULT_ID: i32 = 0;

    #[test]
    fn test_get_tf_of_term() {
        let html = fs::read_to_string(test_file_path_from_filepath("tf.html")).unwrap();
        let document = Document::new(Html::parse_document(html.as_str()), DEFAULT_ID);

        let term = Term::new(String::from("hippopotamus"));

        assert_eq!(term.get_tf(&document.extract_relevant_terms()), 4);
    }

    #[test]
    fn test_extract_terms() {
        let document = Document::new(
            Html::parse_document(
                r#"
            <body>
                <p>hippopotamus hippopotamus hippopotamus</p>
            </body>"#,
            ),
            0,
        );
        let expected_terms = vec![
            Term::new(String::from("hippopotamus")),
            Term::new(String::from("hippopotamus")),
            Term::new(String::from("hippopotamus")),
        ];

        assert_eq!(document.extract_relevant_terms(), expected_terms);
    }

    #[test]
    fn test_add_document() {
        let document = Document::new(
            Html::parse_document(
                r#"
            <body>
                <p>hippopotamus hippopotamus hippopotamus</p>
            </body>"#,
            ),
            0,
        );

        let mut indexer = Indexer::new(HashMap::new());

        indexer.add_document(document.clone());

        assert_eq!(indexer.documents.first().unwrap(), &document);
    }

    #[test]
    fn test_update_idf() {
        let mut term = Term::new(String::from("hippopotamus"));
        term.document_frequency = 2;

        term.clone().update_total_idf(2);

        assert_eq!(term.idf, 0.0);
    }

    #[test]
    fn test_filter_stop_words() {
        let html =
            fs::read_to_string(test_file_path_from_filepath("filter_stop_words.html")).unwrap();
        let document = Document::new(Html::parse_document(html.as_str()), 0);

        let terms = document.extract_relevant_terms();

        let included_terms = vec![
            Term::new(String::from("hippopotamus")),
            Term::new(String::from("ladder")),
        ];

        assert_eq!(terms, included_terms);
    }

    #[test]
    fn test_add_term() {
        let document = Document::new(Html::new_document(), 0);
        let mut term = Term::new(String::from("hippopotamus"));
        term.tf_scores
            .insert(document.clone(), ordered_float::OrderedFloat(0.0));
        term.tf_idf_scores
            .insert(document.clone(), ordered_float::OrderedFloat(0.0));

        let mut indexer = Indexer::new(HashMap::new());

        indexer.add_term(term.clone());

        assert_eq!(indexer.terms.get("hippopotamus").unwrap(), &term);
    }

    #[test]
    fn test_parse_document() {
        let document1 = Document::new(
            Html::parse_document(
                r#"
        <body>
            <p>hippopotamus hippopotamus hippopotamus</p>
        </body>"#,
            ),
            0,
        );

        let document2 = Document::new(
            Html::parse_document(
                r#"
        <body>
            <p>elephant elephant elephant</p>
        </body>"#,
            ),
            1,
        );

        let mut indexer = Indexer::new(HashMap::new());

        indexer.parse_document(document1.clone());
        indexer.parse_document(document2.clone());

        // Hippopotamus term
        let mut expected_hippo = Term::new(String::from("hippopotamus"));
        expected_hippo.idf = ordered_float::OrderedFloat(f32::consts::LOG10_2);
        expected_hippo.document_frequency = 1;
        expected_hippo
            .tf_idf_scores
            .insert(document2.clone(), ordered_float::OrderedFloat(0.0)); // TF = 0 in document2
        expected_hippo
            .tf_idf_scores
            .insert(document1.clone(), ordered_float::OrderedFloat(0.90309)); // TF-IDF in document1

        // Elephant term
        let mut expected_elephant = Term::new(String::from("elephant"));
        expected_elephant.idf = ordered_float::OrderedFloat(f32::consts::LOG10_2);
        expected_elephant.document_frequency = 1;
        expected_elephant
            .tf_idf_scores
            .insert(document1.clone(), ordered_float::OrderedFloat(0.0)); // TF = 0 in document1
        expected_elephant
            .tf_idf_scores
            .insert(document2.clone(), ordered_float::OrderedFloat(0.90309)); // TF-IDF in document2

        let mut expected_terms = HashMap::new();
        expected_terms.insert(expected_hippo.term.clone(), expected_hippo.clone());
        expected_terms.insert(expected_elephant.term.clone(), expected_elephant.clone());

        let expected_terms = vec![expected_hippo, expected_elephant];

        assert_eq!(indexer.num_documents, 2);

        for expected_term in expected_terms {
            let err_msg = &format!("Term '{}' not found in indexer", expected_term.term);
            let term_in_indexer = indexer.terms.get(&expected_term.term).expect(err_msg);

            assert_eq!(
                term_in_indexer.idf, expected_term.idf,
                "IDF mismatch for term '{}'",
                expected_term.term
            );
            assert_eq!(
                term_in_indexer.document_frequency, expected_term.document_frequency,
                "Document frequency mismatch for term '{}'",
                expected_term.term
            );

            for (expected_doc, tf_idf) in &expected_term.tf_idf_scores {
                let err_msg = &format!(
                    "TF-IDF {} not found for term '{}' in document {}, instead found TF-IDF {}",
                    tf_idf, expected_term.term, expected_doc.id, tf_idf
                );
                let (doc, _) = term_in_indexer
                    .tf_idf_scores
                    .get_key_value(expected_doc)
                    .expect(err_msg);
                assert_eq!(
                    doc.id, expected_doc.id,
                    "Document ID mismatch for term '{}'",
                    expected_term.term
                );
            }
        }
    }
}
