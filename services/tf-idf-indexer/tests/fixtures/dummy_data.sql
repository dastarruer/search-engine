INSERT INTO pages (url, html, is_crawled, is_indexed)
VALUES
    ('https://url-0.com', '<body><p>ladder ladder pipe</p></body>', TRUE, FALSE),
    ('https://url-1.com', '<body><p>hippopotamus ladder hippopotamus</p></body>', TRUE, FALSE),
    ('https://url-2.com', '<body><p>ladder hippopotamus hippopotamus</p></body>', TRUE, FALSE);

INSERT INTO terms (term, idf, page_frequency, tf_scores, tf_idf_scores)
VALUES
    ('ladder', 0, 3, '0=>0,1=>0,2=>0', '0=>0,1=>0,2=>0'),
    ('hippopotamus', 0.405465, 2, '0=>0,1=>0.6667,2=>0.6667', '0=>0,1=>0.2703,2=>0.2703'),
    ('pipe', 1.098612, 1, '0=>0.3333,1=>0,2=>0', '0=>0.3662,1=>0,2=>0');
