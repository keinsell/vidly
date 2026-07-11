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
