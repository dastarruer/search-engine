use std::collections::{HashMap, HashSet};

use scraper::Html;
use tf_idf_indexer::{Indexer, Page};

fn main() {
    let mut starting_pages = HashSet::new();

    for i in 0..4 {
        let id = i;

        let page = Page::new(
            Html::parse_document(
                r#"
            <body>
                <p>hippopotamus hippopotamus hippopotamus</p>
            </body>"#,
            ),
            id,
        );

        starting_pages.insert(page);
    }

    let mut indexer = Indexer::new(HashMap::new(), starting_pages);

    indexer.run();
}
