CREATE TABLE movie_tags (
    movie_id INTEGER NOT NULL REFERENCES movies(id),
    tag_id INTEGER NOT NULL REFERENCES tags(id),
    PRIMARY KEY (movie_id, tag_id)
);

INSERT INTO movie_tags (movie_id, tag_id) VALUES
    (1, 5), (1, 4),
    (2, 5), (2, 4),
    (3, 5), (3, 4),
    (4, 5), (4, 4), (4, 3);
