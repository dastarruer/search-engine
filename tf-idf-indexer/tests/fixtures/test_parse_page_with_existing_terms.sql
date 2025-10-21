INSERT INTO pages (url, html, is_crawled, is_indexed)
VALUES
    ('https://url-0.com', '<body><p>ladder ladder pipe</p></body>', TRUE, FALSE),
    ('https://url-1.com', '<body><p>hippopotamus ladder hippopotamus</p></body>', TRUE, FALSE),
    ('https://url-2.com', '<body><p>seagull hippopotamus hippopotamus</p></body>', TRUE, FALSE),
    ('https://url-3.com', '<body><p>seagull</p></body>', TRUE, TRUE);

INSERT INTO terms (term, idf, page_frequency, tf_scores, tf_idf_scores)
VALUES
    ('seagull', 0, 1, '4=>1', '4=>0');
