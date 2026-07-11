use std::collections::HashMap;

use chrono;
use diesel::dsl;
use diesel::prelude::*;
use diesel::sql_types::Integer;
use diesel::sqlite::SqliteConnection;

pub use crate::database::model::tag::{Tag, TagWithRelationships};
use crate::database::model::tag::{NewTag, TagEdge, TagRef};
use crate::database::schema::{tag_edges, tags};

pub fn create_tag(
    conn: &mut SqliteConnection,
    name: String,
    slug: Option<String>,
    description: Option<String>,
    icon: Option<String>,
    thumbnail: Option<String>,
) -> Result<Tag, &'static str> {
    let slug = slug.unwrap_or_else(|| slugify(&name));

    let new_tag = NewTag {
        name,
        slug,
        description,
        icon,
        thumbnail,
        deleted_at: None,
    };

    diesel::insert_into(tags::table)
        .values(&new_tag)
        .execute(conn)
        .map_err(|_| "Database error creating tag")?;

    let last_id: i32 = dsl::select(dsl::sql::<Integer>("last_insert_rowid()"))
        .get_result(conn)
        .map_err(|_| "Database error getting last ID")?;

    tags::table
        .filter(tags::id.eq(last_id))
        .select(Tag::as_select())
        .first::<Tag>(conn)
        .map_err(|_| "Database error fetching created tag")
}

pub fn get_tag(conn: &mut SqliteConnection, id: i32) -> Result<Option<Tag>, &'static str> {
    tags::table
        .filter(tags::id.eq(id))
        .select(Tag::as_select())
        .first::<Tag>(conn)
        .optional()
        .map_err(|_| "Database error fetching tag")
}

pub fn get_tag_by_slug(
    conn: &mut SqliteConnection, slug: &str,
) -> Result<Option<Tag>, &'static str> {
    tags::table
        .filter(tags::slug.eq(slug))
        .select(Tag::as_select())
        .first::<Tag>(conn)
        .optional()
        .map_err(|_| "Database error fetching tag")
}

pub fn list_tags(conn: &mut SqliteConnection) -> Result<Vec<Tag>, &'static str> {
    tags::table
        .select(Tag::as_select())
        .load::<Tag>(conn)
        .map_err(|_| "Database error listing tags")
}

pub fn list_root_tags(conn: &mut SqliteConnection) -> Result<Vec<Tag>, &'static str> {
    let child_ids: Vec<i32> = tag_edges::table
        .select(tag_edges::child_id)
        .load(conn)
        .map_err(|_| "Database error fetching child tag IDs")?;

    let mut root_ids: Vec<i32> = tags::table
        .filter(tags::deleted_at.is_null())
        .select(tags::id)
        .load(conn)
        .map_err(|_| "Database error fetching all tag IDs")?;

    root_ids.retain(|id| !child_ids.contains(id));

    tags::table
        .filter(tags::id.eq_any(root_ids))
        .select(Tag::as_select())
        .load::<Tag>(conn)
        .map_err(|_| "Database error listing root tags")
}

pub fn update_tag(
    conn: &mut SqliteConnection,
    id: i32,
    name: String,
    slug: Option<String>,
    description: Option<String>,
    icon: Option<String>,
    thumbnail: Option<String>,
) -> Result<Tag, &'static str> {
    let slug = slug.unwrap_or_else(|| slugify(&name));

    diesel::update(
        tags::table.filter(tags::id.eq(id).and(tags::deleted_at.is_null())),
    )
    .set((
        tags::name.eq(name),
        tags::slug.eq(slug),
        tags::description.eq(description),
        tags::icon.eq(icon),
        tags::thumbnail.eq(thumbnail),
    ))
    .execute(conn)
    .map_err(|_| "Database error updating tag")?;

    tags::table
        .filter(tags::id.eq(id))
        .select(Tag::as_select())
        .first::<Tag>(conn)
        .map_err(|_| "Database error fetching updated tag")
}

pub fn delete_tag(conn: &mut SqliteConnection, id: i32) -> Result<(), &'static str> {
    let now = chrono::Utc::now().to_rfc3339();

    diesel::update(tags::table.filter(tags::id.eq(id).and(tags::deleted_at.is_null())))
        .set(tags::deleted_at.eq(now))
        .execute(conn)
        .map_err(|_| "Database error deleting tag")?;

    Ok(())
}

pub fn add_parent(
    conn: &mut SqliteConnection, tag_id: i32, parent_id: i32,
) -> Result<(), &'static str> {
    let link = TagEdge { child_id: tag_id, parent_id };

    diesel::insert_into(tag_edges::table)
        .values(&link)
        .execute(conn)
        .map_err(|_| "Database error linking tag parent")?;

    Ok(())
}

pub fn remove_parent(
    conn: &mut SqliteConnection, tag_id: i32, parent_id: i32,
) -> Result<(), &'static str> {
    diesel::delete(
        tag_edges::table.filter(
            tag_edges::child_id
                .eq(tag_id)
                .and(tag_edges::parent_id.eq(parent_id)),
        ),
    )
    .execute(conn)
    .map_err(|_| "Database error unlinking tag parent")?;

    Ok(())
}

pub fn get_parents(
    conn: &mut SqliteConnection, tag_id: i32,
) -> Result<Vec<Tag>, &'static str> {
    let parent_ids: Vec<i32> = tag_edges::table
        .filter(tag_edges::child_id.eq(tag_id))
        .select(tag_edges::parent_id)
        .load(conn)
        .map_err(|_| "Database error fetching tag parents")?;

    if parent_ids.is_empty() {
        return Ok(Vec::new());
    }

    tags::table
        .filter(tags::id.eq_any(parent_ids).and(tags::deleted_at.is_null()))
        .select(Tag::as_select())
        .load::<Tag>(conn)
        .map_err(|_| "Database error fetching tag parents")
}

pub fn get_children(
    conn: &mut SqliteConnection, tag_id: i32,
) -> Result<Vec<Tag>, &'static str> {
    let child_ids: Vec<i32> = tag_edges::table
        .filter(tag_edges::parent_id.eq(tag_id))
        .select(tag_edges::child_id)
        .load(conn)
        .map_err(|_| "Database error fetching tag children")?;

    if child_ids.is_empty() {
        return Ok(Vec::new());
    }

    tags::table
        .filter(tags::id.eq_any(child_ids).and(tags::deleted_at.is_null()))
        .select(Tag::as_select())
        .load::<Tag>(conn)
        .map_err(|_| "Database error fetching tag children")
}

pub fn list_tags_with_relationships(
    conn: &mut SqliteConnection,
) -> Result<Vec<TagWithRelationships>, &'static str> {
    let all_tags: Vec<Tag> = tags::table
        .filter(tags::deleted_at.is_null())
        .select(Tag::as_select())
        .load::<Tag>(conn)
        .map_err(|_| "Database error listing tags")?;

    let all_parents: Vec<TagEdge> = tag_edges::table
        .select(TagEdge::as_select())
        .load::<TagEdge>(conn)
        .map_err(|_| "Database error fetching tag relationships")?;

    let tag_map: HashMap<i32, &Tag> = all_tags.iter().map(|t| (t.id, t)).collect();

    let mut parent_map: HashMap<i32, Vec<i32>> = HashMap::new();
    let mut child_map: HashMap<i32, Vec<i32>> = HashMap::new();

    for rel in &all_parents {
        child_map.entry(rel.parent_id).or_default().push(rel.child_id);
        parent_map.entry(rel.child_id).or_default().push(rel.parent_id);
    }

    let result = all_tags
        .iter()
        .map(|tag| {
            let parents = parent_map
                .get(&tag.id)
                .map(|ids| {
                    ids.iter()
                        .filter_map(|id| tag_map.get(id))
                        .map(|t| TagRef { name: t.name.clone(), slug: t.slug.clone() })
                        .collect()
                })
                .unwrap_or_default();

            let children = child_map
                .get(&tag.id)
                .map(|ids| {
                    ids.iter()
                        .filter_map(|id| tag_map.get(id))
                        .map(|t| TagRef { name: t.name.clone(), slug: t.slug.clone() })
                        .collect()
                })
                .unwrap_or_default();

            TagWithRelationships { tag: tag.clone(), parents, children }
        })
        .collect();

    Ok(result)
}

pub fn get_tag_with_relationships(
    conn: &mut SqliteConnection, id: i32,
) -> Result<Option<TagWithRelationships>, &'static str> {
    let tag: Option<Tag> = tags::table
        .filter(tags::id.eq(id).and(tags::deleted_at.is_null()))
        .select(Tag::as_select())
        .first::<Tag>(conn)
        .optional()
        .map_err(|_| "Database error fetching tag")?;

    let tag = match tag {
        Some(t) => t,
        None => return Ok(None),
    };

    let parent_ids: Vec<i32> = tag_edges::table
        .filter(tag_edges::child_id.eq(tag.id))
        .select(tag_edges::parent_id)
        .load(conn)
        .map_err(|_| "Database error fetching tag parents")?;

    let child_ids: Vec<i32> = tag_edges::table
        .filter(tag_edges::parent_id.eq(tag.id))
        .select(tag_edges::child_id)
        .load(conn)
        .map_err(|_| "Database error fetching tag children")?;

    let parents = if parent_ids.is_empty() {
        Vec::new()
    } else {
        tags::table
            .filter(tags::id.eq_any(parent_ids).and(tags::deleted_at.is_null()))
            .select((tags::name, tags::slug))
            .load::<(String, String)>(conn)
            .map_err(|_| "Database error fetching parent tags")?
            .into_iter()
            .map(|(name, slug)| TagRef { name, slug })
            .collect()
    };

    let children = if child_ids.is_empty() {
        Vec::new()
    } else {
        tags::table
            .filter(tags::id.eq_any(child_ids).and(tags::deleted_at.is_null()))
            .select((tags::name, tags::slug))
            .load::<(String, String)>(conn)
            .map_err(|_| "Database error fetching child tags")?
            .into_iter()
            .map(|(name, slug)| TagRef { name, slug })
            .collect()
    };

    Ok(Some(TagWithRelationships { tag, parents, children }))
}

pub fn slugify(input: &str) -> String {
    let mut slug: Vec<String> = input
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    if slug.is_empty() {
        slug.push("untitled".to_string());
    }

    slug.join("-")
}
