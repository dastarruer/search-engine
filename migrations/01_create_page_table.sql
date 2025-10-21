CREATE TABLE IF NOT EXISTS pages (
    id serial NOT NULL, -- serial is autoincrementing
    url text NOT NULL UNIQUE,
    html text NOT NULL,
    is_indexed bool NOT NULL,
    http_status smallint NOT NULL, -- Store the http error code
    PRIMARY KEY (id) -- Makes the id the identifier for each row
);
