use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::database::schema::{movie_tags, tag_edges, tags};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TagRef {
    pub name: String,
    pub slug: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TagWithRelationships {
    #[serde(flatten)]
    pub tag: Tag,
    pub parents: Vec<TagRef>,
    pub children: Vec<TagRef>,
}

#[derive(Queryable, Selectable, Clone, Debug, Serialize, Deserialize)]
#[diesel(table_name = tags)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Tag {
	pub id: i32,
	pub name: String,
	pub slug: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub thumbnail: Option<String>,
	pub deleted_at: Option<String>,
}

#[derive(Insertable, Clone, Debug)]
#[diesel(table_name = tags)]
pub struct NewTag {
	pub name: String,
	pub slug: String,
	pub description: Option<String>,
	pub icon: Option<String>,
	pub thumbnail: Option<String>,
	pub deleted_at: Option<String>,
}

#[derive(Insertable, Clone, Debug, Queryable, Selectable)]
#[diesel(table_name = tag_edges)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct TagEdge {
	pub child_id: i32,
	pub parent_id: i32,
}

#[derive(Insertable, Clone, Debug, Queryable, Selectable, Associations)]
#[diesel(table_name = movie_tags)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(belongs_to(crate::database::model::movie::Movie))]
#[diesel(belongs_to(Tag))]
pub struct MovieTag {
	pub movie_id: i32,
	pub tag_id: i32,
}
