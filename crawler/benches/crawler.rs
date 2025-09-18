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
        })
    });
}

criterion_group! {
    name = benches;
    // Sacrifice time for more consistent benchmarks
    config = Criterion::default()
        .sample_size(150)
        .measurement_time(std::time::Duration::from_secs(15))
        .warm_up_time(std::time::Duration::from_secs(5))
        .nresamples(200_000);
    targets = bench_crawl_from_page, bench_test_run
}

criterion_main!(benches);
