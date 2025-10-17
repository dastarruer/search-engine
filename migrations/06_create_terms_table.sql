-- Necessary for the hstore attribute, which is used for key value pairs
CREATE EXTENSION hstore;

CREATE TABLE terms (
    id serial primary key NOT NULL,
    term text NOT NULL UNIQUE,
    idf double precision NOT NULL,
    page_frequency integer NOT NULL,
    tf_scores hstore,
    tf_idf_scores hstore
);
