CREATE TABLE tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    description TEXT,
    icon TEXT,
    thumbnail TEXT,
    deleted_at TEXT
);

CREATE UNIQUE INDEX idx_tags_slug ON tags (slug);

CREATE TABLE tag_edges (
    child_id INTEGER NOT NULL REFERENCES tags(id),
    parent_id INTEGER NOT NULL REFERENCES tags(id),
    PRIMARY KEY (child_id, parent_id)
);

-- Seed parent tags (id auto-generated)
INSERT INTO tags (name, slug, description) VALUES
('Entertainment', 'entertainment', 'Movies, TV, music and interactive media'),
('Education', 'education', 'Learning content, tutorials, and academic material'),
('Technology', 'technology', 'Computing, software, and digital systems'),
('Creative Arts', 'creative-arts', 'Visual arts, design, animation and craftsmanship');

-- Seed child tags (id auto-generated)
INSERT INTO tags (name, slug, description) VALUES
('Film & TV', 'film-tv', 'Feature films, television series and video productions'),
('Music', 'music', 'Musical performances, compositions and theory'),
('Gaming', 'gaming', 'Video games, gameplay and game culture'),
('Tutorials', 'tutorials', 'Step-by-step instructional content'),
('Science & Math', 'science-math', 'Scientific principles and mathematical concepts'),
('Programming', 'programming', 'Software development and engineering'),
('AI & ML', 'ai-ml', 'Artificial intelligence and machine learning'),
('Web Development', 'web-development', 'Frontend, backend and full-stack web technologies'),
('Animation', 'animation', 'Traditional and computer-generated animation'),
('Design', 'design', 'Graphic design, UI/UX and visual communication'),
('Visual Arts', 'visual-arts', 'Painting, photography and digital art'),
('Game Development', 'game-development', 'Design and development of video games');

-- Link parent-child relationships (resolve IDs by slug)
INSERT INTO tag_edges (child_id, parent_id)
SELECT c.id, p.id FROM tags c, tags p WHERE
    (c.slug = 'film-tv' AND p.slug = 'entertainment') OR
    (c.slug = 'music' AND p.slug = 'entertainment') OR
    (c.slug = 'gaming' AND p.slug = 'entertainment') OR
    (c.slug = 'tutorials' AND p.slug = 'education') OR
    (c.slug = 'science-math' AND p.slug = 'education') OR
    (c.slug = 'programming' AND p.slug = 'technology') OR
    (c.slug = 'ai-ml' AND p.slug = 'technology') OR
    (c.slug = 'web-development' AND p.slug = 'technology') OR
    (c.slug = 'animation' AND p.slug = 'creative-arts') OR
    (c.slug = 'design' AND p.slug = 'creative-arts') OR
    (c.slug = 'visual-arts' AND p.slug = 'creative-arts') OR
    (c.slug = 'game-development' AND p.slug = 'gaming') OR
    (c.slug = 'game-development' AND p.slug = 'technology');
