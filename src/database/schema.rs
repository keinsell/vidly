diesel::table! {
	movies (id) {
		id -> Integer,
		title -> Text,
		description -> Text,
		thumb -> Text,
		sources -> Text,
	}
}
