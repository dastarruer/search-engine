use crawler::crawler::Crawler;
use criterion::{Criterion, criterion_group, criterion_main};
use url::Url;

fn bench_crawl_from_url(c: &mut Criterion) {
    let url = Url::parse("https://books.toscrape.com/").unwrap();
    c.bench_function("crawl_from_url", |b| {
        b.iter(async || {
            let mut crawler = Crawler::new(url.clone());
            crawler.crawl_from_url(url.clone()).await.unwrap();
        })
    });
}

criterion_group!(benches, bench_crawl_from_url);
criterion_main!(benches);
