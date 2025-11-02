use anyhow::anyhow;
use sqlx::{Row, postgres::types::PgHstore};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Instant,
};
use utils::{AddToDb, ExtractText};

use once_cell::sync::Lazy;
use ordered_float::OrderedFloat;
use scraper::Html;

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
    page_frequency: u32,

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

impl TryFrom<String> for Term {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Term::new(
            value,
            OrderedFloat(0.0),
            0,
            PgHstore::default(),
            PgHstore::default(),
        )
    }
}

// TODO: Convert back to From, since a term in the database is guaranteed to be a valid term
impl TryFrom<&sqlx::postgres::PgRow> for Term {
    type Error = anyhow::Error;

    fn try_from(value: &sqlx::postgres::PgRow) -> Result<Self, Self::Error> {
        let term: String = value.get("term");
        let idf = OrderedFloat(value.get("idf"));
        let page_frequency = value.get::<i32, _>("page_frequency") as u32;
        let tf_scores: PgHstore = value.get("tf_scores");
        let tf_idf_scores: PgHstore = value.get("tf_idf_scores");

        Term::new(term, idf, page_frequency, tf_scores, tf_idf_scores)
    }
}

impl Term {
    /// Create a new instance of [`Term`].
    ///
    /// # Parameters
    /// - `term` - The term to be stored. This gets normalized, meaning
    ///   whitespace and punctuation is trimmed, diacritics are removed (e.g. `é` becomes `e`), and the term is converted to
    ///   lowercase.
    /// - `idf` - The Inverse Document Frequency of a term. See [`Term::idf`]
    ///   for more information.
    /// - `page_frequency` - Measures how many pages this term is found in. Used
    ///   to calculate Inverse Document Frequency.
    /// - `tf_scores` — A mapping of page IDs to the *Term Frequency* (TF) for
    ///   this term in each page. Pages with a term frequency of `0` should not be included, since they are not worth storing.
    /// - `tf_idf_scores` — A mapping of page IDs to their *TF-IDF* score for
    ///   this term. TF-IDF is computed as `Term Frequency × Inverse Document
    ///   Frequency`. Pages with a term frequency of `0` should not be
    ///   included, since they are not worth storing.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The term provided contains numerical characters. This can be a number
    ///   (e.g. `1220`), or a `String` containing a number (e.g. `hello123`).
    pub fn new(
        term: String,
        idf: ordered_f32,
        page_frequency: u32,
        tf_scores: PgHstore,
        tf_idf_scores: PgHstore,
    ) -> Result<Self, anyhow::Error> {
        let term = diacritics::remove_diacritics(term.as_str());
        if term
            .chars()
            .any(|c| !c.is_alphabetic() && !c.is_ascii_punctuation())
        {
            return Err::<Self, anyhow::Error>(anyhow!(
                "Term contains invalid characters that are neither punctuation nor letters: {}",
                term
            ));
        }

        // Normalize the term
        let term = term
            .to_lowercase()
            .trim()
            .chars()
            .filter(|c| !c.is_ascii_punctuation())
            .collect();

        log::debug!("Term: {}", term);
        Ok(Term {
            term,
            idf,
            page_frequency,
            tf_scores,
            tf_idf_scores,
        })
    }

    /// Find the number of times that a [`Term`] appears in a given piece of
    /// text.
    ///
    /// This is called the *term frequency* of a term. This is useful when
    /// calculating the TF-IDF score of a term, which is used to check how
    /// frequent a [`Term`] is in one page, and how rare it is in other
    /// pages.
    fn get_tf(&self, terms: &[Term]) -> u32 {
        terms
            .iter()
            .filter(|t| t.term.eq_ignore_ascii_case(&self.term))
            .count() as u32
    }

    /// Update [`Term::page_frequency`] based on given term frequency.
    ///
    /// Increments [`Term::page_frequency`] if the term appears at least once.
    fn update_page_frequency(&mut self, tf: u32) {
        // If the term appears at least once, incrememnt page frequency
        if tf > 0 {
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
            .bind(self.page_frequency as i32)
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
    pub async fn new(pool: &sqlx::Pool<sqlx::Postgres>) -> Indexer {
        let starting_terms: HashMap<i32, Term> = HashMap::new();

        let num_pages = 0;

        let mut indexer = Indexer {
            terms: HashMap::new(),
            pages: PageQueue::new(HashSet::new()),
            num_pages,
        };

        // Add starting terms
        for (_, term) in starting_terms {
            indexer.add_term(term, None).await;
        }

        let num_pages_query = r#"SELECT COUNT(*) FROM pages WHERE is_indexed = TRUE;"#;
        indexer.num_pages = sqlx::query_scalar(num_pages_query)
            .fetch_one(pool)
            .await
            .expect("Fetching page count should not throw an error.");

        // Add pages from the db
        log::info!("Populating page queue...");
        indexer.refresh_queue(pool).await;

        indexer
    }

    pub async fn run(&mut self, pool: &sqlx::PgPool) {
        while let Some(page) = self.pages.pop() {
            log::info!("Parsing page {}...", page.id);
            self.parse_page(page, Some(pool)).await;

            if self.pages.queue.is_empty() {
                log::info!("Page queue is empty...");

                for (_, term) in self.terms.iter_mut() {
                    term.update_tf_idf_scores();
                }

                log::info!("Updating terms in the database...");
                self.update_terms_in_db(pool).await;

                log::info!("Clearing terms in memory...");
                self.empty_terms();

                log::info!("Refreshing page queue...");
                self.refresh_queue(pool).await;
            }
        }

        log::info!("All pages are indexed, exiting...");
    }

    /// Refresh the queue by reading from the database.
    ///
    /// If there are no pages currently in the database, then keep looping
    /// until pages are found.
    pub async fn refresh_queue(&mut self, pool: &sqlx::PgPool) {
        // TODO: Remove loop during tests
        // loop {
        let query = format!(
            r#"SELECT id, html FROM pages WHERE is_indexed = FALSE AND is_crawled = TRUE LIMIT {};"#,
            utils::QUEUE_LIMIT
        );

        sqlx::query(query.as_str())
            .fetch_all(pool)
            .await
            .unwrap()
            .iter()
            .for_each(|row| {
                self.add_page(Page::from(row));
            });

        // if !self.pages.is_empty() {
        //     break;
        // }

        //     log::info!("No pages found in the database, trying again in 10 seconds...");
        //     sleep(Duration::from_secs(10));
        // }
        log::info!("Queue is refreshed!");
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
            self.add_term(term.clone(), pool).await;
        }

        // Loop through each stored term
        for (_, term) in self.terms.iter_mut() {
            let tf = term.get_tf(&relevant_terms);

            term.update_page_frequency(tf);

            term.update_total_idf(self.num_pages);

            // Only update tf_scores if the term appears in this page
            if tf > 0 {
                term.tf_scores
                    .insert(page.id.to_string(), Some(tf.to_string()));
            }

            // Instead of updating tf-idf scores at the end here, we update them in the run method
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

    async fn update_terms_in_db(&mut self, pool: &sqlx::PgPool) {
        // yes as much as this sucks I can't really see any other way to update every single term's idf and tf-idf scores
        let term_query = r#"SELECT * FROM terms;"#;

        let db_terms: HashMap<String, Term> = sqlx::query(term_query)
            .fetch_all(pool)
            .await
            .expect("Fetching terms should not throw an error")
            .iter()
            .flat_map(|row| {
                if let Ok(old_term) = Term::try_from(row) {
                    // If the term is in memory (aka it has a more recent version), then merge the old and new terms
                    if let Some(new_term) = self.terms.get(&old_term.term).cloned() {
                        return Some((old_term.term.clone(), self.merge_terms(old_term, new_term)));
                    }
                    // Otherwise, just return the old term so its tf-idf scores can be updated
                    Some((old_term.term.clone(), old_term))
                } else {
                    None
                }
            })
            .collect();

        // Add terms from the db to the terms in memory
        // Since self.terms is a hashset, db_terms will overwrite all duplicates
        self.terms.extend(db_terms);

        // Then add every term to the database
        for term in self.terms.values() {
            term.add_to_db(pool).await;
        }
    }

    fn merge_terms(&self, old_term: Term, new_term: Term) -> Term {
        assert_eq!(old_term.term, new_term.term);

        let merged_term_str = old_term.term;
        let mut merged_term = Term::try_from(merged_term_str).expect(
            "Creating a `Term` instance while merging two terms should not throw an error.",
        );

        // We can assume new_term will have the higher page frequency since new_term should be the most recent term
        merged_term.page_frequency = new_term.page_frequency;
        merged_term.idf = new_term.idf;

        // Again, new_term should be the most recent term
        merged_term.tf_scores = new_term.tf_scores;

        // Then, add the tf scores from old_term that were missing in new_term
        for (page_id, score) in old_term.tf_scores {
            merged_term.tf_scores.entry(page_id).or_insert(score);
        }

        merged_term.tf_idf_scores = new_term.tf_idf_scores;
        // Then, add the tf-idf scores from old_term that were missing in new_term
        for (page_id, score) in old_term.tf_idf_scores {
            merged_term.tf_idf_scores.entry(page_id).or_insert(score);
        }

        merged_term.update_tf_idf_scores();

        merged_term
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
            return Some(
                Term::try_from(&row)
                    .expect("Terms stored in the database should be valid, alphabetical terms."),
            );
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
    id: u32,
    html: Html,
}

impl From<&sqlx::postgres::PgRow> for Page {
    fn from(value: &sqlx::postgres::PgRow) -> Self {
        let html = value.get("html");
        let id = value.get::<i32, _>("id") as u32;

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
    pub fn new(html: Html, id: u32) -> Self {
        Page { html, id }
    }

    /// Extract relevant [`Term`]s from [`Html`].
    ///
    /// First filters out common 'stop words' (see [`Term::is_stop_word`] for more information), and then returns the resulting list of [`Term`]s.
    fn extract_relevant_terms(&self) -> Vec<Term> {
        self.html
            .extract_text()
            .split_whitespace()
            .flat_map(|t: &str| Term::try_from(t.to_string()))
            .filter(|t| !t.is_stop_word())
            .collect()
    }

    async fn mark_as_crawled(self, pool: &sqlx::PgPool) {
        sqlx::query("UPDATE pages SET is_indexed = TRUE WHERE id = $1")
            .bind(self.id as i32)
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

    use crate::{Indexer, Page, PageQueue, Term, test_file_path_from_filepath};

    const DEFAULT_ID: u32 = 0;

    impl Default for Indexer {
        fn default() -> Self {
            Indexer {
                terms: HashMap::new(),
                pages: PageQueue::new(HashSet::new()),
                num_pages: 0,
            }
        }
    }

    mod merge_terms {
        use super::*;

        #[test]
        fn test_merge_terms() {
            let old_term = Term::new(
                "hippopotamus".to_string(),
                OrderedFloat(1.5),
                10,
                PgHstore::from_iter([("1".into(), Some("1".into()))]),
                PgHstore::from_iter([("1".into(), Some("1.5".into()))]),
            )
            .unwrap();

            let new_term = Term::new(
                "hippopotamus".to_string(),
                OrderedFloat(2.5),
                15,
                PgHstore::from_iter([("2".into(), Some("1".into()))]),
                PgHstore::from_iter([("2".into(), Some("2.5".into()))]),
            )
            .unwrap();

            let indexer = Indexer::default();

            let merged = indexer.merge_terms(old_term, new_term);

            assert_eq!(merged.page_frequency, 15);
            // Firstly, the tf score in term a should be present
            assert_eq!(
                merged.tf_scores.get_key_value("1").unwrap(),
                (&String::from("1"), &Some(String::from("1")))
            );

            // Then, the tf score in term b should be present
            assert_eq!(
                merged.tf_scores.get_key_value("2").unwrap(),
                (&String::from("2"), &Some(String::from("1")))
            );

            // Firstly, the tf-idf score in term a should be present, with its newly calculated value
            assert_eq!(
                merged.tf_idf_scores.get_key_value("1").unwrap(),
                (&String::from("1"), &Some(String::from("2.5")))
            );

            // Then, the tf score in term b should be present with its old value
            assert_eq!(
                merged.tf_idf_scores.get_key_value("2").unwrap(),
                (&String::from("2"), &Some(String::from("2.5")))
            );
        }

        #[test]
        fn test_merge_conflicting_terms() {
            let old_term = Term::new(
                "hippopotamus".to_string(),
                OrderedFloat(1.5),
                10,
                PgHstore::from_iter([
                    ("1".into(), Some("1".into())),
                    ("2".into(), Some("1".into())),
                ]),
                PgHstore::from_iter([
                    ("1".into(), Some("1.5".into())),
                    ("2".into(), Some("1.5".into())),
                ]),
            )
            .unwrap();

            let new_term = Term::new(
                "hippopotamus".to_string(),
                OrderedFloat(2.5),
                15,
                PgHstore::from_iter([
                    ("2".into(), Some("1".into())),
                    ("3".into(), Some("1".into())),
                ]),
                PgHstore::from_iter([
                    ("2".into(), Some("2.5".into())),
                    ("3".into(), Some("2.5".into())),
                ]),
            )
            .unwrap();

            let indexer = Indexer::default();

            let merged = indexer.merge_terms(old_term, new_term);

            assert_eq!(merged.page_frequency, 15);

            // The tf scores in term a & b should be present
            assert_eq!(
                merged.tf_scores.get_key_value("1").unwrap(),
                (&String::from("1"), &Some(String::from("1")))
            );
            assert_eq!(
                merged.tf_scores.get_key_value("2").unwrap(),
                (&String::from("2"), &Some(String::from("1")))
            );
            assert_eq!(
                merged.tf_scores.get_key_value("3").unwrap(),
                (&String::from("3"), &Some(String::from("1")))
            );

            // The tf-idf scores of term a & b should be present, along with their newly calculated values
            assert_eq!(
                merged.tf_idf_scores.get_key_value("1").unwrap(),
                (&String::from("1"), &Some(String::from("2.5")))
            );
            assert_eq!(
                merged.tf_idf_scores.get_key_value("2").unwrap(),
                (&String::from("2"), &Some(String::from("2.5")))
            );
            assert_eq!(
                merged.tf_idf_scores.get_key_value("3").unwrap(),
                (&String::from("3"), &Some(String::from("2.5")))
            );
        }
    }

    mod new_term {
        use super::*;

        #[test]
        #[should_panic]
        fn test_nonalphabetical_term() {
            Term::try_from(String::from("123")).unwrap();
        }

        #[test]
        #[should_panic]
        fn test_nonalphabetical_term_with_alphabetical_chars() {
            Term::try_from(String::from("abc123")).unwrap();
        }

        #[test]
        fn test_term_with_punctuation() {
            let term = Term::try_from(String::from("abc-?>")).unwrap();
            assert_eq!(term.term, "abc");
        }
    }

    #[test]
    fn test_get_tf_of_term() {
        let html = fs::read_to_string(test_file_path_from_filepath("tf.html")).unwrap();
        let page = Page::new(Html::parse_document(html.as_str()), DEFAULT_ID);

        let term = Term::try_from(String::from("hippopotamus")).unwrap();

        assert_eq!(term.get_tf(&page.extract_relevant_terms()), 4);
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
            Term::try_from(String::from("hippopotamus")).unwrap(),
            Term::try_from(String::from("hippopotamus")).unwrap(),
            Term::try_from(String::from("hippopotamus")).unwrap(),
        ];

        assert_eq!(page.extract_relevant_terms(), expected_terms);
    }

    mod update_page_frequency {
        use super::*;

        #[test]
        fn test_positive_nonzero_tf() {
            let mut term = Term::try_from(String::from("hippopotamus")).unwrap();

            // A hypothetical term frequency
            let tf = 2;

            term.update_page_frequency(tf);

            assert_eq!(term.page_frequency, 1);
        }

        #[test]
        fn test_zero_tf() {
            let mut term = Term::try_from(String::from("hippopotamus")).unwrap();

            // A hypothetical term frequency
            let tf = 0;

            term.update_page_frequency(tf);

            assert_eq!(term.page_frequency, 0);
        }
    }

    mod add_page {
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

            let mut indexer = Indexer::default();

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

            let mut indexer = Indexer::default();

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

        let mut term = Term::try_from(String::from("hippopotamus")).unwrap();

        // Manually set up TF for both pages
        let tf1 = 2;
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
            (OrderedFloat(tf1 as f32) * OrderedFloat::<f32>(0.0)).to_string(),
        )]);

        assert_eq!(term.tf_idf_scores, expected_tf_idf);

        let page2 = Page::new(Html::parse_document("<body><p>ladder</p></body>"), 1);

        let tf2 = 0;

        term.tf_scores
            .insert(page2.id.to_string(), Some(tf2.to_string()));

        term.update_page_frequency(tf2);

        // Update idf, which should be log(2/1), where 1 is the number of
        // pages and 1 is the number of pages the term is found in
        term.idf = OrderedFloat(f32::consts::LOG10_2);

        term.update_tf_idf_scores();

        expected_tf_idf.insert(
            page1.id.to_string(),
            Some((OrderedFloat(tf1 as f32) * OrderedFloat(f32::consts::LOG10_2)).to_string()),
        );
        expected_tf_idf.insert(
            page2.id.to_string(),
            Some((OrderedFloat(tf2 as f32) * OrderedFloat(f32::consts::LOG10_2)).to_string()),
        );

        assert_eq!(term.tf_idf_scores, expected_tf_idf);
    }

    mod update_idf {
        use super::*;

        #[test]
        fn test_update_idf() {
            let mut term = Term::try_from(String::from("hippopotamus")).unwrap();
            term.page_frequency = 2;

            term.clone().update_total_idf(2);

            assert_eq!(term.idf, 0.0);
        }

        #[test]
        fn test_zero_doc_frequency() {
            let mut term = Term::try_from(String::from("hippopotamus")).unwrap();
            term.page_frequency = 0;

            term.update_total_idf(2);

            assert_eq!(term.idf, 0.0);
        }

        #[test]
        fn test_zero_num_pages() {
            let mut term = Term::try_from(String::from("hippopotamus")).unwrap();
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
            Term::try_from(String::from("hippopotamus")).unwrap(),
            Term::try_from(String::from("ladder")).unwrap(),
        ];

        assert_eq!(terms, included_terms);
    }

    #[tokio::test]
    async fn test_add_term() {
        let page = Page::new(Html::new_document(), 0);
        let mut term = Term::try_from(String::from("hippopotamus")).unwrap();
        term.tf_scores
            .insert(page.id.to_string(), Some(OrderedFloat(0.0).to_string()));
        term.tf_idf_scores
            .insert(page.id.to_string(), Some(OrderedFloat(0.0).to_string()));

        let mut indexer = Indexer::default();

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

        let mut indexer = Indexer::default();

        indexer.add_page(page1.clone());
        indexer.parse_page(page1.clone(), None).await;

        indexer.add_page(page2.clone());
        indexer.parse_page(page2.clone(), None).await;

        // Hippopotamus term
        let mut expected_hippo = Term::try_from(String::from("hippopotamus")).unwrap();
        expected_hippo.idf = OrderedFloat(f32::consts::LOG10_2);
        expected_hippo.page_frequency = 1;
        expected_hippo.tf_idf_scores.insert(
            page1.id.to_string(),
            Some(OrderedFloat(0.90309).to_string()),
        );

        // Elephant term
        let mut expected_elephant = Term::try_from(String::from("elephant")).unwrap();
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
            let term_in_indexer = indexer.terms.get_mut(&expected_term.term).expect(err_msg);

            // Do this manually since this is actually updated in the run method for efficiency, not the parse_page method
            term_in_indexer.update_tf_idf_scores();

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
