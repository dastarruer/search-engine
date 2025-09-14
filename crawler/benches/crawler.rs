use crawler::{crawler::Crawler, page::Page, utils::HttpServer};
use criterion::{Criterion, criterion_group, criterion_main};

/// Benchmark crawling a single page
fn bench_crawl_from_page(c: &mut Criterion) {
    let server = HttpServer::new("benchmarks/index.html");

    let page = Page::from(server.base_url());
    let mut crawler = None;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all() // Will panic, for some reason...
        .build()
        .unwrap();

    // A fun workaround for benching async functions in Criterion:
    // https://stackoverflow.com/questions/77601738/adding-async-functions-in-criterion-group-macro
    runtime.block_on(async {
        crawler = Some(Crawler::new(page.clone()).await);
    });

    c.bench_function("crawl_from_page", |b| {
        b.to_async(&runtime).iter(|| async {
            let mut crawler = crawler.clone().unwrap();

            crawler.crawl_page(page.clone()).await.unwrap();
            crawler.reset();
        })
    });
}

/// Benchmark crawling an entire site
fn bench_test_run(c: &mut Criterion) {
    let server = HttpServer::new("benchmarks/index.html");

    let page = Page::from(server.base_url());
    let mut crawler = None;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all() // Will panic, for some reason...
        .build()
        .unwrap();

    // A fun workaround for benching async functions in Criterion:
    // https://stackoverflow.com/questions/77601738/adding-async-functions-in-criterion-group-macro
    runtime.block_on(async {
        crawler = Some(Crawler::new(page.clone()).await);
    });

    c.bench_function("test_run", |b| {
        b.to_async(&runtime).iter(|| async {
            let mut crawler = crawler.clone().unwrap();

            crawler.test_run().await.unwrap();
            crawler.reset();
        })
    });
}

criterion_group!(benches, bench_crawl_from_page, bench_test_run);
criterion_main!(benches);
