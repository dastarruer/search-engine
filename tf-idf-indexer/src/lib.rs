pub mod utils;

use std::collections::{HashMap, HashSet, VecDeque};

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
pub struct Term {
    pub term: String,

    /// The Inverse Document Frequency of a term.
    ///
    /// This measures how rare a term is across documents (which are referred to as pages here). If the term appears in many pages, then the IDF is low. If the term only appears in one or two pages, the IDF is high.
    idf: ordered_f32,

    /// The amount of pages that contain this term. Used for calculating [`Term::idf`].
    page_frequency: i32,

    /// The TF scores of each [`Page`].
    ///
    /// TF is measured as the term frequency of a [`Term`], or how many times a term appears in a given [`Page`].
    tf_scores: HashMap<Page, ordered_f32>,

    /// The TF-IDF scores of each [`Page`].
    ///
    /// TF-IDF is measured as the term frequency of a [`Term`] in a [`Page`] multiplied by [`Term::idf`].
    tf_idf_scores: HashMap<Page, ordered_f32>,
}

// Manually implement the Hash trait since HashMap does not implement Hash
impl std::hash::Hash for Term {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Just hash the term instead of anything else
        self.term.to_lowercase().hash(state);
    }
}

impl Term {
    pub fn new(term: String) -> Self {
        Term {
            term,
            idf: ordered_float::OrderedFloat(0.0),
            page_frequency: 0,
            tf_scores: HashMap::new(),
            tf_idf_scores: HashMap::new(),
        }
    }

    /// Find the number of times that a [`Term`] appears in a given piece of text.
    ///
    /// This is called the *term frequency* of a term. This is useful when
    /// calculating the TF-IDF score of a term, which is used to check how
    /// frequent a [`Term`] is in one page, and how rare it is in other
    /// pages.
    fn get_tf<'b>(&self, terms: &Vec<Term>) -> ordered_f32 {
        ordered_float::OrderedFloat(
            terms
                .iter()
                .filter(|t| t.term.eq_ignore_ascii_case(&self.term))
                .count() as f32,
        )
    }

    /// Update [`Term::page_frequency`] based on given term frequency.
    ///
    /// Increments [`Term::page_frequency`] if the term appears at least once.
    fn update_page_frequency(&mut self, tf: OrderedFloat<f32>) {
        // If the term appears at least once, incrememnt page frequency
        if tf > ordered_float::OrderedFloat(0.0) {
            self.page_frequency += 1;
        }
    }

    /// Update the IDF score of a [`Term`] (see [`Term::idf`] for more details).
    ///
    /// This is useful when calculating the TF-IDF score of a term, which is
    /// used to check how frequent a [`Term`] is in one page, and how rare
    /// it is in other pages.
    fn update_total_idf(&mut self, num_pages: i32) {
        // Prevent divide-by-zero error
        if self.page_frequency == 0 {
            self.idf = ordered_float::OrderedFloat(0.0);
            return;
        }

        let idf = num_pages as f32 / self.page_frequency as f32;
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

    /// Updates all TF-IDF scores for this term across every page.
    ///
    /// Should be called whenever [`Term::idf`] changes. TF-IDF is calculated
    /// as term frequency * IDF, which needs to be refreshed for every page
    /// if IDF ever changes.
    fn update_tf_idf_scores(&mut self) {
        for (page, tf) in self.tf_scores.iter_mut() {
            let new_tf_idf = tf.clone() * self.idf;
            self.tf_idf_scores.insert(page.clone(), new_tf_idf);
        }
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

#[derive(Clone, Debug, PartialEq)]
pub struct PageQueue {
    queue: VecDeque<Page>,
    hashset: HashSet<Page>,
}

impl PageQueue {
    fn new(pages: HashSet<Page>) -> Self {
        PageQueue {
            queue: pages.clone().into_iter().collect(),
            hashset: pages,
        }
    }

    fn contains(&self, page: &Page) -> bool {
        self.hashset.contains(page)
    }

    fn push(&mut self, page: Page) {
        self.queue.push_back(page.clone());
        self.hashset.insert(page);
    }

    fn pop(&mut self) -> Option<Page> {
        if let Some(page) = self.queue.front() {
            self.hashset.remove(page);
            self.queue.pop_front()
        } else {
            None
        }
    }
}

impl<'a> IntoIterator for &'a PageQueue {
    type Item = &'a Page;
    type IntoIter = std::collections::vec_deque::Iter<'a, Page>;

    fn into_iter(self) -> Self::IntoIter {
        self.queue.iter()
    }
}

pub struct Indexer {
    terms: HashMap<String, Term>,
    pages: PageQueue,
    num_pages: i32,
}

impl Indexer {
    pub fn new(starting_terms: HashMap<String, Term>, starting_pages: HashSet<Page>) -> Self {
        let num_pages = starting_pages.len() as i32;

        let mut indexer = Indexer {
            terms: HashMap::new(),
            pages: PageQueue::new(starting_pages),
            num_pages,
        };

        // Add starting terms
        for (_, term) in starting_terms {
            indexer.add_term(term);
        }

        indexer
    }

    pub fn run(&mut self) {
        let mut i = 0;
        while let Some(page) = self.pages.pop() {
            self.parse_page(page);
            println!("Page {} parsed", i);
            i += 1;
        }

        println!("All done!");
    }

    fn parse_page(&mut self, page: Page) {
        let relevant_terms = page.extract_relevant_terms();

        for term in relevant_terms.clone() {
            self.add_term(term);
        }

        // Loop through each stored term
        for (_, term) in self.terms.iter_mut() {
            let tf = term.get_tf(&relevant_terms);

            term.update_page_frequency(tf);

            term.update_total_idf(self.num_pages);

            let tf_idf = tf * term.idf;

            term.tf_scores.insert(page.clone(), tf);
            term.tf_idf_scores.insert(page.clone(), tf_idf);

            // Go back and update the tf_idf scores for every other single page
            term.update_tf_idf_scores();
        }
    }

    /// Add a new [`Page`] to the set of existing pages, and increment
    /// [`Indexer::num_pages`].
    ///
    /// Does not add a duplicate page.
    // TODO: Maybe this shouldn't panic...?
    fn add_page(&mut self, page: Page) {
        if !self.pages.contains(&page) {
            self.pages.push(page);
            self.num_pages += 1;
        };
    }

    fn add_term(&mut self, term: Term) {
        if !self.terms.contains_key(&term.term) {
            let mut new_term = term.clone();

            // Initialize tf and tf_idf for all existing pages
            for doc in &self.pages {
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
pub struct Page {
    id: i32,
    html: Html,
}

impl PartialEq for Page {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

// Manually implement the Hash trait since Html does not implement Hash
impl std::hash::Hash for Page {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Just hash the id, since it's supposed to be unique
        self.id.hash(state);
    }
}

impl Page {
    pub fn new(html: Html, id: i32) -> Self {
        Page { html, id }
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
    use std::{
        collections::{HashMap, HashSet},
        f32, fs,
    };

    use scraper::Html;

    use crate::{Indexer, Page, Term, test_file_path_from_filepath};

    const DEFAULT_ID: i32 = 0;

    #[test]
    fn test_get_tf_of_term() {
        let html = fs::read_to_string(test_file_path_from_filepath("tf.html")).unwrap();
        let page = Page::new(Html::parse_document(html.as_str()), DEFAULT_ID);

        let term = Term::new(String::from("hippopotamus"));

        assert_eq!(
            term.get_tf(&page.extract_relevant_terms()),
            ordered_float::OrderedFloat(4.0)
        );
    }

    #[test]
    fn test_extract_terms() {
        let page = Page::new(
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

        assert_eq!(page.extract_relevant_terms(), expected_terms);
    }

    mod update_page_frequency {
        use super::*;

        #[test]
        fn test_positive_nonzero_tf() {
            let mut term = Term::new(String::from("hippopotamus"));

            // A hypothetical term frequency
            let tf = ordered_float::OrderedFloat(2.0);

            term.update_page_frequency(tf);

            assert_eq!(term.page_frequency, 1);
        }

        #[test]
        fn test_zero_tf() {
            let mut term = Term::new(String::from("hippopotamus"));

            // A hypothetical term frequency
            let tf = ordered_float::OrderedFloat(0.0);

            term.update_page_frequency(tf);

            assert_eq!(term.page_frequency, 0);
        }
    }

    mod add_page {
        use std::collections::HashSet;

        use crate::PageQueue;

        use super::*;

        impl PageQueue {
            fn get(&self, page: &Page) -> Option<&Page> {
                self.hashset.get(page)
            }
        }

        #[test]
        fn test_add_page() {
            let page = Page::new(
                Html::parse_document(
                    r#"
                <body>
                    <p>hippopotamus hippopotamus hippopotamus</p>
                </body>"#,
                ),
                0,
            );

            let mut indexer = Indexer::new(HashMap::new(), HashSet::new());

            indexer.add_page(page.clone());

            assert_eq!(indexer.pages.get(&page).unwrap(), &page);
        }

        #[test]
        fn test_add_duplicate_page() {
            let page = Page::new(
                Html::parse_document(
                    r#"
                <body>
                    <p>hippopotamus hippopotamus hippopotamus</p>
                </body>"#,
                ),
                0,
            );

            let mut indexer = Indexer::new(HashMap::new(), HashSet::new());

            indexer.add_page(page.clone());
            indexer.add_page(page.clone());

            assert_eq!(indexer.pages.queue.len(), 1);
        }
    }

    #[test]
    fn test_update_tf_idf_scores() {
        let page1 = Page::new(
            Html::parse_document("<body><p>hippopotamus hippopotamus</p></body>"),
            0,
        );

        let mut term = Term::new(String::from("hippopotamus"));

        // Manually set up TF for both pages
        let tf1 = ordered_float::OrderedFloat(2.0);
        term.tf_scores.insert(page1.clone(), tf1);

        term.update_page_frequency(tf1);

        // Update idf, which should be log(1/1), where 1 is the number of
        // pages and 1 is the number of pages the term is found in
        term.idf = ordered_float::OrderedFloat(0.0);

        // Update the TF-IDF scores based on the new idf
        term.update_tf_idf_scores();

        // Expected TF-IDF values
        let mut expected_tf_idf = HashMap::new();
        expected_tf_idf.insert(page1.clone(), tf1 * ordered_float::OrderedFloat(0.0));

        assert_eq!(term.tf_idf_scores, expected_tf_idf);

        let page2 = Page::new(Html::parse_document("<body><p>ladder</p></body>"), 1);

        let tf2 = ordered_float::OrderedFloat(0.0);

        term.tf_scores.insert(page2.clone(), tf2);

        term.update_page_frequency(tf2);

        // Update idf, which should be log(2/1), where 1 is the number of
        // pages and 1 is the number of pages the term is found in
        term.idf = ordered_float::OrderedFloat(f32::consts::LOG10_2);

        term.update_tf_idf_scores();

        expected_tf_idf.insert(
            page1.clone(),
            tf1 * ordered_float::OrderedFloat(f32::consts::LOG10_2),
        );
        expected_tf_idf.insert(
            page2.clone(),
            tf2 * ordered_float::OrderedFloat(f32::consts::LOG10_2),
        );

        assert_eq!(term.tf_idf_scores, expected_tf_idf);
    }

    mod update_idf {
        use super::*;

        #[test]
        fn test_update_idf() {
            let mut term = Term::new(String::from("hippopotamus"));
            term.page_frequency = 2;

            term.clone().update_total_idf(2);

            assert_eq!(term.idf, 0.0);
        }

        #[test]
        fn test_zero_doc_frequency() {
            let mut term = Term::new(String::from("hippopotamus"));
            term.page_frequency = 0;

            term.update_total_idf(2);

            assert_eq!(term.idf, 0.0);
        }
    }

    #[test]
    fn test_filter_stop_words() {
        let html =
            fs::read_to_string(test_file_path_from_filepath("filter_stop_words.html")).unwrap();
        let page = Page::new(Html::parse_document(html.as_str()), 0);

        let terms = page.extract_relevant_terms();

        let included_terms = vec![
            Term::new(String::from("hippopotamus")),
            Term::new(String::from("ladder")),
        ];

        assert_eq!(terms, included_terms);
    }

    #[test]
    fn test_add_term() {
        let page = Page::new(Html::new_document(), 0);
        let mut term = Term::new(String::from("hippopotamus"));
        term.tf_scores
            .insert(page.clone(), ordered_float::OrderedFloat(0.0));
        term.tf_idf_scores
            .insert(page.clone(), ordered_float::OrderedFloat(0.0));

        let mut indexer = Indexer::new(HashMap::new(), HashSet::new());

        indexer.add_term(term.clone());

        assert_eq!(indexer.terms.get("hippopotamus").unwrap(), &term);
    }

    #[test]
    fn test_parse_document() {
        let page1 = Page::new(
            Html::parse_document(
                r#"
        <body>
            <p>hippopotamus hippopotamus hippopotamus</p>
        </body>"#,
            ),
            0,
        );

        let page2 = Page::new(
            Html::parse_document(
                r#"
        <body>
            <p>elephant elephant elephant</p>
        </body>"#,
            ),
            1,
        );

        let mut indexer = Indexer::new(HashMap::new(), HashSet::new());

        indexer.parse_page(page1.clone());
        indexer.parse_page(page2.clone());

        // Hippopotamus term
        let mut expected_hippo = Term::new(String::from("hippopotamus"));
        expected_hippo.idf = ordered_float::OrderedFloat(f32::consts::LOG10_2);
        expected_hippo.page_frequency = 1;
        expected_hippo
            .tf_idf_scores
            .insert(page2.clone(), ordered_float::OrderedFloat(0.0)); // TF = 0 in page2
        expected_hippo
            .tf_idf_scores
            .insert(page1.clone(), ordered_float::OrderedFloat(0.90309)); // TF-IDF in page1

        // Elephant term
        let mut expected_elephant = Term::new(String::from("elephant"));
        expected_elephant.idf = ordered_float::OrderedFloat(f32::consts::LOG10_2);
        expected_elephant.page_frequency = 1;
        expected_elephant
            .tf_idf_scores
            .insert(page1.clone(), ordered_float::OrderedFloat(0.0)); // TF = 0 in page1
        expected_elephant
            .tf_idf_scores
            .insert(page2.clone(), ordered_float::OrderedFloat(0.90309)); // TF-IDF in page2

        let mut expected_terms = HashMap::new();
        expected_terms.insert(expected_hippo.term.clone(), expected_hippo.clone());
        expected_terms.insert(expected_elephant.term.clone(), expected_elephant.clone());

        let expected_terms = vec![expected_hippo, expected_elephant];

        assert_eq!(indexer.num_pages, 2);

        for expected_term in expected_terms {
            let err_msg = &format!("Term '{}' not found in indexer", expected_term.term);
            let term_in_indexer = indexer.terms.get(&expected_term.term).expect(err_msg);

            assert_eq!(
                term_in_indexer.idf, expected_term.idf,
                "IDF mismatch for term '{}'",
                expected_term.term
            );
            assert_eq!(
                term_in_indexer.page_frequency, expected_term.page_frequency,
                "page frequency mismatch for term '{}'",
                expected_term.term
            );

            for (expected_doc, tf_idf) in &expected_term.tf_idf_scores {
                let err_msg = &format!(
                    "TF-IDF {} not found for term '{}' in page {}, instead found TF-IDF {}",
                    tf_idf, expected_term.term, expected_doc.id, tf_idf
                );
                let (doc, _) = term_in_indexer
                    .tf_idf_scores
                    .get_key_value(expected_doc)
                    .expect(err_msg);
                assert_eq!(
                    doc.id, expected_doc.id,
                    "page ID mismatch for term '{}'",
                    expected_term.term
                );
            }
        }
    }
}
