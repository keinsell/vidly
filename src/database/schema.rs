diesel::table! {
	movies (id) {
		id -> Integer,
		title -> Text,
		description -> Text,
		subtitle -> Text,
		thumb -> Text,
		sources -> Text,
	}
}
