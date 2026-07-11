diesel::table! {
	movies (id) {
		id -> Integer,
		title -> Text,
		description -> Text,
		thumb -> Text,
		sources -> Text,
	}
}

diesel::table! {
	tags (id) {
		id -> Integer,
		name -> Text,
		slug -> Text,
		description -> Nullable<Text>,
		icon -> Nullable<Text>,
		thumbnail -> Nullable<Text>,
		deleted_at -> Nullable<Text>,
	}
}

diesel::table! {
	tag_edges (child_id, parent_id) {
		child_id -> Integer,
		parent_id -> Integer,
	}
}

diesel::table! {
	movie_tags (movie_id, tag_id) {
		movie_id -> Integer,
		tag_id -> Integer,
	}
}

diesel::joinable!(movie_tags -> movies (movie_id));
diesel::joinable!(movie_tags -> tags (tag_id));
