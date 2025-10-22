use sqlx::{Row, postgres::types::PgHstore};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Instant,
};
use utils::AddToDb;

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
type ordered_f32 = OrderedFloat<helper::f32_helper>;

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Term {
    pub term: String,

    /// The Inverse Document Frequency of a term, calculated as the total
    /// number of documents indexed divided by the number of documents the term
    /// appears in.
    ///
    /// This measures how rare a term is across documents (which are referred
    /// to as pages here). If the term appears in many pages, then the IDF is
    /// low. If the term only appears in one or two pages, the IDF is high.
    idf: ordered_f32,

    /// The amount of pages that contain this term. Used for calculating [`Term::idf`].
    page_frequency: i32,

    /// The TF scores of each [`Page`], stored as <[`Page::id`]>:`<TF of term in a page>`.
    ///
    /// TF is measured as the term frequency of a [`Term`], or how many times a
    /// term appears in a given [`Page`].
    tf_scores: PgHstore,

    /// The TF-IDF scores of each [`i32`], stored as <[`Page::id`]>:<TF-IDF of term in a [`Page`]>.
    ///
    /// TF-IDF is measured as the term frequency of a [`Term`] in a [`Page`] multiplied by [`Term::idf`].
    tf_idf_scores: PgHstore,
}

// Manually implement the Hash trait since HashMap does not implement Hash
impl std::hash::Hash for Term {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Just hash the term instead of anything else
        self.term.to_lowercase().hash(state);
    }
}

impl From<String> for Term {
    fn from(value: String) -> Self {
        Term::new(
            value,
            OrderedFloat(0.0),
            0,
            PgHstore::default(),
            PgHstore::default(),
        )
    }
}

impl From<&sqlx::postgres::PgRow> for Term {
    fn from(value: &sqlx::postgres::PgRow) -> Self {
        let term: String = value.get("term");
        let idf = OrderedFloat(value.get("idf"));
        let page_frequency: i32 = value.get("page_frequency");
        let tf_scores: PgHstore = value.get("tf_scores");
        let tf_idf_scores: PgHstore = value.get("tf_idf_scores");

        Term::new(term, idf, page_frequency, tf_scores, tf_idf_scores)
    }
}

impl Term {
    pub fn new(
        term: String,
        idf: ordered_f32,
        page_frequency: i32,
        tf_scores: PgHstore,
        tf_idf_scores: PgHstore,
    ) -> Self {
        Term {
            term,
            idf,
            page_frequency,
            tf_scores,
            tf_idf_scores,
        }
    }

    /// Find the number of times that a [`Term`] appears in a given piece of text.
    ///
    /// This is called the *term frequency* of a term. This is useful when
    /// calculating the TF-IDF score of a term, which is used to check how
    /// frequent a [`Term`] is in one page, and how rare it is in other
    /// pages.
    fn get_tf(&self, terms: &[Term]) -> ordered_f32 {
        OrderedFloat(
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
        if tf > OrderedFloat(0.0) {
            self.page_frequency += 1;
        }
    }

    /// Update the IDF score of a [`Term`] (see [`Term::idf`] for more details).
    ///
    /// This is useful when calculating the TF-IDF score of a term, which is
    /// used to check how frequent a [`Term`] is in one page, and how rare
    /// it is in other pages.
    fn update_total_idf(&mut self, num_pages: i64) {
        // Prevent divide-by-zero error or evaluating log(0)
        if self.page_frequency == 0 || num_pages == 0 {
            self.idf = OrderedFloat(0.0);
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
        for (page_id, tf) in self.tf_scores.iter() {
            let tf = OrderedFloat(
                tf.as_ref()
                    .expect("Every page should have a TF score for every term.")
                    .parse::<f32>()
                    .expect("Every TF score should be a valid f32 value."),
            );
            let new_tf_idf = tf * self.idf;

            self.tf_idf_scores
                .insert(page_id.to_owned(), Some(new_tf_idf.to_string()));
        }
    }
}

impl AddToDb for Term {
    /// Add a [`Term`] instance to a database.
    async fn add_to_db(&self, pool: &sqlx::PgPool) {
        // This query tries to insert the term and its values into a new row.
        // But if the term already exists, then it updates the existing term's
        // values instead.
        let query = r#"
            INSERT INTO terms (term, idf, page_frequency, tf_scores, tf_idf_scores)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (term)
            DO UPDATE SET
                idf = EXCLUDED.idf,
                page_frequency = EXCLUDED.page_frequency,
                tf_scores = EXCLUDED.tf_scores,
                tf_idf_scores = EXCLUDED.tf_idf_scores
        "#;

        sqlx::query(query)
            .bind(&self.term)
            .bind(*self.idf) // Dereferencing gives the inner f32 value
            .bind(self.page_frequency)
            .bind(&self.tf_scores)
            .bind(&self.tf_idf_scores)
            .execute(pool)
            .await
            .unwrap();
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
    num_pages: i64,
}

impl Indexer {
    pub async fn new(starting_terms: HashMap<String, Term>, starting_pages: HashSet<Page>) -> Self {
        let num_pages = starting_pages.len() as i64;

        let mut indexer = Indexer {
            terms: HashMap::new(),
            pages: PageQueue::new(starting_pages),
            num_pages,
        };

        // Add starting terms
        for (_, term) in starting_terms {
            indexer.add_term(term, None).await;
        }

        indexer
    }

    pub async fn new_with_pool(pool: &sqlx::Pool<sqlx::Postgres>) -> Indexer {
        let starting_terms = HashMap::new();

        let mut indexer = Indexer::new(starting_terms, HashSet::new()).await;

        let num_pages_query = r#"SELECT COUNT(*) FROM pages WHERE is_indexed = TRUE;"#;
        indexer.num_pages = sqlx::query_scalar(num_pages_query)
            .fetch_one(pool)
            .await
            .expect("Fetching page count should not throw an error.");

        // Add pages from the db
        indexer.refresh_queue(pool).await;

        indexer
    }

    pub async fn run(&mut self, pool: &sqlx::PgPool) {
        while let Some(page) = self.pages.pop() {
            log::info!("Parsing page {}...", page.id);
            self.parse_page(page, Some(pool)).await;

            for term in self.terms.values() {
                log::debug!("Adding/updating term: {}", term.term);
                term.add_to_db(pool).await;
            }
            self.empty_terms();

            if self.pages.queue.is_empty() {
                log::info!("Page queue is empty, refreshing...");
                self.refresh_queue(pool).await;
            }
        }

        println!("All done!");
    }

    /// Refresh the queue by reading from the database.
    pub async fn refresh_queue(&mut self, pool: &sqlx::PgPool) {
        let query = r#"SELECT id, html FROM pages WHERE is_indexed = FALSE AND is_crawled = TRUE LIMIT 100;"#;

        sqlx::query(query)
            .fetch_all(pool)
            .await
            .unwrap()
            .iter()
            .for_each(|row| {
                self.add_page(Page::from(row));
            });
    }

    /// Parse a [`Page`], extracting relevant terms and adding them to
    /// [`Indexer::terms`], and optionally add them to the database.
    ///
    /// # Parameters
    /// - `page: Page` - The page to be parsed.
    /// - `pool: Option<&sqlx::PgPool>` - The connection to the database. If
    ///   passed as `None`, then terms are not added to or read from the
    ///   database. This is useful when testing. If passed as `Some`, then
    ///   terms are added to and read from the database.
    async fn parse_page(&mut self, page: Page, pool: Option<&sqlx::PgPool>) {
        let start_time = Instant::now();

        let mut relevant_terms = page.extract_relevant_terms();

        log::info!("Found {} terms in page {}", relevant_terms.len(), page.id);

        for term in relevant_terms.iter_mut() {
            log::debug!("Found term: {}", term.term);
            // TODO: this method is not correctly retrieving the term from the db, and therefore the page frequency is off(?)
            self.add_term(term.clone(), pool).await;
        }

        if let Some(pool) = pool {
            // yes as much as this sucks I can't really see any other way to update every single term's idf and tf-idf scores
            let term_query = r#"SELECT * FROM terms;"#;

            sqlx::query(term_query)
                .fetch_all(pool)
                .await
                .expect("Fetching terms should not throw an error")
                .iter()
                .for_each(|row| {
                    self.terms.insert(row.get("term"), Term::from(row));
                });
        }

        // Loop through each stored term
        for (_, term) in self.terms.iter_mut() {
            let tf = term.get_tf(&relevant_terms);

            term.update_page_frequency(tf);

            term.update_total_idf(self.num_pages);

            // Only update tf_scores if the term appears in this page
            if tf > OrderedFloat(0.0) {
                term.tf_scores
                    .insert(page.id.to_string(), Some(tf.to_string()));
            }

            // Go back and update the tf_idf scores for every other single page
            term.update_tf_idf_scores();
        }

        let duration = start_time.elapsed();
        if let Some(pool) = pool {
            log::info!(
                "Page {} indexed successfully in {:.2?}! Marking as indexed...",
                page.id,
                duration
            );
            page.mark_as_crawled(pool).await;
            return;
        }

        log::info!("Page {} indexed successfully in {:.2?}!", page.id, duration);
    }

    fn empty_terms(&mut self) {
        self.terms.clear();
    }

    /// Returns the number of [`Page`] instances in the indexer.
    pub fn num_pages(&self) -> i64 {
        self.num_pages
    }

    /// Returns `True` if the page is stored in the indexer.
    ///
    /// Returns `False` if the page is not stored in the indexer.
    pub fn contains_page(&self, page: &Page) -> bool {
        self.pages.contains(page)
    }

    /// Returns `True` if the term is stored in the indexer.
    ///
    /// Returns `False` if the term is not stored in the indexer.
    pub fn contains_term(&self, term: &Term) -> bool {
        self.terms.contains_key(&term.term)
    }

    /// Add a new [`Page`] to the set of existing pages, and increment
    /// [`Indexer::num_pages`].
    ///
    /// Does not add a duplicate page.
    fn add_page(&mut self, page: Page) {
        if !self.pages.contains(&page) {
            self.pages.push(page);
            self.num_pages += 1;
        };
    }

    pub async fn add_term(&mut self, term: Term, pool: Option<&sqlx::PgPool>) {
        let key = term.term.clone();

        // If already in memory, skip, since we don’t need to reload or replace it.
        if self.terms.contains_key(&key) {
            return;
        }

        // Try fetching from database if pool is available.
        if let Some(pool) = pool
            && let Some(db_term) = Self::get_term_from_db(pool, &term).await
        {
            self.terms.insert(key.clone(), db_term.clone());
            return;
        }

        // Otherwise, insert the term that was passed.
        self.terms.insert(key.clone(), term.clone());
    }

    async fn get_term_from_db(pool: &sqlx::PgPool, term: &Term) -> Option<Term> {
        let query = r#"SELECT * FROM terms WHERE term = $1"#;

        if let Ok(Some(row)) = sqlx::query(query)
            .bind(&term.term)
            .fetch_optional(pool)
            .await
        {
            return Some(Term::from(&row));
        }
        None
    }
}

/// Return the path of a file in src/fixtures given just its filename.
#[cfg(test)]
pub fn test_file_path_from_filepath(filename: &str) -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR gets the source dir of the project
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("fixtures")
        .join(filename)
}

#[derive(Eq, Debug, Clone)]
pub struct Page {
    id: i32,
    html: Html,
}

impl From<&sqlx::postgres::PgRow> for Page {
    fn from(value: &sqlx::postgres::PgRow) -> Self {
        let html = value.get("html");
        // TODO: Decrement this by 1 so the ids are zero indexed maybe?
        let id = value.get("id");

        Page::new(Html::parse_document(html), id)
    }
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
    // TODO: Extract text extraction into seperate method and add better testing
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
            .map(|t: String| Term::from(t))
            .filter(|t| !t.is_stop_word())
            .collect()
    }

    async fn mark_as_crawled(self, pool: &sqlx::PgPool) {
        sqlx::query("UPDATE pages SET is_indexed = TRUE WHERE id = $1")
            .bind(self.id)
            .execute(pool)
            .await
            .unwrap();
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::{HashMap, HashSet},
        f32, fs,
    };

    use ordered_float::OrderedFloat;
    use scraper::Html;
    use sqlx::postgres::types::PgHstore;

    use crate::{Indexer, Page, Term, test_file_path_from_filepath};

    const DEFAULT_ID: i32 = 0;

    #[test]
    fn test_get_tf_of_term() {
        let html = fs::read_to_string(test_file_path_from_filepath("tf.html")).unwrap();
        let page = Page::new(Html::parse_document(html.as_str()), DEFAULT_ID);

        let term = Term::from(String::from("hippopotamus"));

        assert_eq!(
            term.get_tf(&page.extract_relevant_terms()),
            OrderedFloat(4.0)
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
            Term::from(String::from("hippopotamus")),
            Term::from(String::from("hippopotamus")),
            Term::from(String::from("hippopotamus")),
        ];

        assert_eq!(page.extract_relevant_terms(), expected_terms);
    }

    mod update_page_frequency {
        use super::*;

        #[test]
        fn test_positive_nonzero_tf() {
            let mut term = Term::from(String::from("hippopotamus"));

            // A hypothetical term frequency
            let tf = OrderedFloat(2.0);

            term.update_page_frequency(tf);

            assert_eq!(term.page_frequency, 1);
        }

        #[test]
        fn test_zero_tf() {
            let mut term = Term::from(String::from("hippopotamus"));

            // A hypothetical term frequency
            let tf = OrderedFloat(0.0);

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

        #[tokio::test]
        async fn test_add_page() {
            let page = Page::new(
                Html::parse_document(
                    r#"
                <body>
                    <p>hippopotamus hippopotamus hippopotamus</p>
                </body>"#,
                ),
                0,
            );

            let mut indexer = Indexer::new(HashMap::new(), HashSet::new()).await;

            indexer.add_page(page.clone());

            assert_eq!(indexer.pages.get(&page).unwrap(), &page);
        }

        #[tokio::test]
        async fn test_add_duplicate_page() {
            let page = Page::new(
                Html::parse_document(
                    r#"
                <body>
                    <p>hippopotamus hippopotamus hippopotamus</p>
                </body>"#,
                ),
                0,
            );

            let mut indexer = Indexer::new(HashMap::new(), HashSet::new()).await;

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

        let mut term = Term::from(String::from("hippopotamus"));

        // Manually set up TF for both pages
        let tf1 = OrderedFloat(2.0);
        term.tf_scores
            .insert(page1.id.to_string(), Some(tf1.to_string()));

        term.update_page_frequency(tf1);

        // Update idf, which should be log(1/1), where 1 is the number of
        // pages and 1 is the number of pages the term is found in
        term.idf = OrderedFloat(0.0);

        // Update the TF-IDF scores based on the new idf
        term.update_tf_idf_scores();

        // Expected TF-IDF values
        let mut expected_tf_idf = PgHstore::from_iter([(
            page1.id.to_string(),
            (tf1 * OrderedFloat::<f32>(0.0)).to_string(),
        )]);

        assert_eq!(term.tf_idf_scores, expected_tf_idf);

        let page2 = Page::new(Html::parse_document("<body><p>ladder</p></body>"), 1);

        let tf2 = OrderedFloat(0.0);

        term.tf_scores
            .insert(page2.id.to_string(), Some(tf2.to_string()));

        term.update_page_frequency(tf2);

        // Update idf, which should be log(2/1), where 1 is the number of
        // pages and 1 is the number of pages the term is found in
        term.idf = OrderedFloat(f32::consts::LOG10_2);

        term.update_tf_idf_scores();

        expected_tf_idf.insert(
            page1.id.to_string(),
            Some((tf1 * OrderedFloat(f32::consts::LOG10_2)).to_string()),
        );
        expected_tf_idf.insert(
            page2.id.to_string(),
            Some((tf2 * OrderedFloat(f32::consts::LOG10_2)).to_string()),
        );

        assert_eq!(term.tf_idf_scores, expected_tf_idf);
    }

    mod update_idf {
        use super::*;

        #[test]
        fn test_update_idf() {
            let mut term = Term::from(String::from("hippopotamus"));
            term.page_frequency = 2;

            term.clone().update_total_idf(2);

            assert_eq!(term.idf, 0.0);
        }

        #[test]
        fn test_zero_doc_frequency() {
            let mut term = Term::from(String::from("hippopotamus"));
            term.page_frequency = 0;

            term.update_total_idf(2);

            assert_eq!(term.idf, 0.0);
        }

        #[test]
        fn test_zero_num_pages() {
            let mut term = Term::from(String::from("hippopotamus"));
            term.page_frequency = 2;

            term.update_total_idf(0);

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
            Term::from(String::from("hippopotamus")),
            Term::from(String::from("ladder")),
        ];

        assert_eq!(terms, included_terms);
    }

    #[tokio::test]
    async fn test_add_term() {
        let page = Page::new(Html::new_document(), 0);
        let mut term = Term::from(String::from("hippopotamus"));
        term.tf_scores
            .insert(page.id.to_string(), Some(OrderedFloat(0.0).to_string()));
        term.tf_idf_scores
            .insert(page.id.to_string(), Some(OrderedFloat(0.0).to_string()));

        let mut indexer = Indexer::new(HashMap::new(), HashSet::new()).await;

        indexer.add_term(term.clone(), None).await;

        assert_eq!(indexer.terms.get("hippopotamus").unwrap(), &term);
    }

    #[tokio::test]
    async fn test_parse_page() {
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

        let mut indexer = Indexer::new(HashMap::new(), HashSet::new()).await;

        indexer.add_page(page1.clone());
        indexer.parse_page(page1.clone(), None).await;

        indexer.add_page(page2.clone());
        indexer.parse_page(page2.clone(), None).await;

        // Hippopotamus term
        let mut expected_hippo = Term::from(String::from("hippopotamus"));
        expected_hippo.idf = OrderedFloat(f32::consts::LOG10_2);
        expected_hippo.page_frequency = 1;
        expected_hippo.tf_idf_scores.insert(
            page1.id.to_string(),
            Some(OrderedFloat(0.90309).to_string()),
        );

        // Elephant term
        let mut expected_elephant = Term::from(String::from("elephant"));
        expected_elephant.idf = OrderedFloat(f32::consts::LOG10_2);
        expected_elephant.page_frequency = 1;
        expected_elephant.tf_idf_scores.insert(
            page2.id.to_string(),
            Some(OrderedFloat(0.90309).to_string()),
        ); 

        let mut expected_terms = HashMap::new();
        expected_terms.insert(expected_hippo.term.clone(), expected_hippo.clone());
        expected_terms.insert(expected_elephant.term.clone(), expected_elephant.clone());

        let expected_terms = vec![expected_hippo, expected_elephant];

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

            for (expected_page_id, tf_idf) in expected_term.tf_idf_scores {
                let err_msg = &format!(
                    "TF-IDF {:?} not found for term '{}' in page {}, instead found TF-IDF {:?}",
                    tf_idf, expected_term.term, expected_page_id, tf_idf
                );

                let (page_id, _) = term_in_indexer
                    .tf_idf_scores
                    .get_key_value(&expected_page_id)
                    .expect(err_msg);

                assert_eq!(
                    *page_id, expected_page_id,
                    "page ID mismatch for term '{}'",
                    expected_term.term
                );
            }
        }
    }
}
