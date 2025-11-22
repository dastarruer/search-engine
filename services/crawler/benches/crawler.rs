use crawler::{crawler::Crawler, page::Page, utils::HttpServer};
use criterion::{Criterion, criterion_group, criterion_main};

/// Benchmark crawling a single page
fn bench_crawl_page(c: &mut Criterion) {
    // TODO: Move this setup code to a separate method
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all() // Will panic, for some reason...
        .build()
        .expect("Creating tokio runtime should not throw an error.");

    let index = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("files")
        .join("index.html");

    c.bench_function("crawl_from_page", |b| {
        b.to_async(&runtime).iter(|| async {
            let server = HttpServer::new_with_filepath(index.clone());

            let page = Page::from(server.base_url());

            let crawler = Crawler::test_new(vec![page.clone()]).await;

            Crawler::crawl_page(page.clone(), crawler.context)
                .await
                .expect("`crawl_page` method should not throw an error.");
        })
    });
}

/// Benchmark crawling an entire site
fn bench_test_run(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all() // Will panic, for some reason...
        .build()
        .expect("Creating tokio runtime should not throw an error.");

    let index = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("files")
        .join("index.html");

    c.bench_function("test_run", |b| {
        b.to_async(&runtime).iter(|| async {
            let server = HttpServer::new_with_filepath(index.clone());

            let page = Page::from(server.base_url());

            let mut crawler = Crawler::test_new(vec![page.clone()]).await;

            let _ = crawler.run().await;
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
    targets = bench_crawl_page, bench_test_run
}

criterion_main!(benches);
