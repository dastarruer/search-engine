use crawler::{crawler::Crawler, page::Page, utils::HttpServer};
use criterion::{Criterion, criterion_group, criterion_main};

/// Benchmark crawling a single page
fn bench_crawl_from_page(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all() // Will panic, for some reason...
        .build()
        .unwrap();

    c.bench_function("crawl_from_page", |b| {
        b.to_async(&runtime).iter(|| async {
            let server = HttpServer::new_with_filename("benchmarks/index.html");

            let page = Page::from(server.base_url());

            let mut crawler = Crawler::test_new(page.clone());

            crawler.crawl_page(page.clone()).await.unwrap();
            crawler.reset();
        })
    });
}

/// Benchmark crawling an entire site
fn bench_test_run(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all() // Will panic, for some reason...
        .build()
        .unwrap();

    c.bench_function("test_run", |b| {
        b.to_async(&runtime).iter(|| async {
            let server = HttpServer::new_with_filename("benchmarks/index.html");

            let page = Page::from(server.base_url());

            let mut crawler = Crawler::test_new(page.clone());

            crawler.test_run().await.unwrap();
            crawler.reset();
        })
    });
}

criterion_group!(benches, bench_crawl_from_page, bench_test_run);
criterion_main!(benches);
