CREATE TABLE movies (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    subtitle TEXT NOT NULL DEFAULT '',
    thumb TEXT NOT NULL DEFAULT '',
    sources TEXT NOT NULL DEFAULT '[]'
);

INSERT INTO movies (id, title, description, subtitle, thumb, sources)
VALUES (
    1,
    'Big Buck Bunny',
    'Big Buck Bunny tells the story of a giant rabbit with a heart bigger than himself.',
    'By Blender Foundation',
    'https://upload.wikimedia.org/wikipedia/commons/c/c5/Big_buck_bunny_poster_big.jpg',
    '["https://download.blender.org/peach/bigbuckbunny_movies/BigBuckBunny_320x180.mp4"]'
);

INSERT INTO movies (id, title, description, subtitle, thumb, sources)
VALUES (
    2,
    'Elephants Dream',
    'The first Blender Open Movie from 2006',
    'By Blender Foundation',
    'https://upload.wikimedia.org/wikipedia/commons/0/0c/ElephantsDreamPoster.jpg',
    '["https://download.blender.org/ED/elephantsdream-480-h264-st-aac.mov"]'
);

INSERT INTO movies (id, title, description, subtitle, thumb, sources)
VALUES (
    3,
    'Sintel',
    'An independently produced short film by Blender Foundation.',
    'By Blender Foundation',
    'https://upload.wikimedia.org/wikipedia/commons/8/8f/Sintel_poster.jpg',
    '["https://download.blender.org/durian/trailer/sintel_trailer-480p.mp4"]'
);

INSERT INTO movies (id, title, description, subtitle, thumb, sources)
VALUES (
    4,
    'Tears of Steel',
    'A crowd-funded sci-fi film realized with Blender.',
    'By Blender Foundation',
    'https://upload.wikimedia.org/wikipedia/commons/7/70/Tos-poster.png',
    '["https://download.blender.org/demo/movies/tears-of-steel_teaser.mp4"]'
);
