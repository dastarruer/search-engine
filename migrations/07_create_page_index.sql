CREATE INDEX IF NOT EXISTS idx_pages_crawled_indexed
    ON pages (is_crawled, is_indexed);
