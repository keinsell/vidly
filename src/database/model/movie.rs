use std::ops::{Deref, DerefMut};
use std::slice::Iter;

use diesel::deserialize::{self, FromSql};
use diesel::expression::AsExpression;
use diesel::prelude::*;
use diesel::serialize::{self, ToSql};
use diesel::sql_types::Text;
use diesel::sqlite::Sqlite;
use serde::{Deserialize, Serialize};

use crate::database::schema::movies;

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    diesel::deserialize::FromSqlRow,
    AsExpression,
)]
#[diesel(sql_type = Text)]
pub struct Sources(pub Vec<String>);

impl Deref for Sources {
    type Target = Vec<String>;

    fn deref(&self) -> &Vec<String> {
        &self.0
    }
}

impl DerefMut for Sources {
    fn deref_mut(&mut self) -> &mut Vec<String> {
        &mut self.0
    }
}

impl<'a> IntoIterator for &'a Sources {
    type Item = &'a String;
    type IntoIter = Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromSql<Text, Sqlite> for Sources {
    fn from_sql(
        bytes: <Sqlite as diesel::backend::Backend>::RawValue<'_>,
    ) -> deserialize::Result<Self> {
        let s = <String as FromSql<Text, Sqlite>>::from_sql(bytes)?;
        serde_json::from_str(&s).map(Sources).map_err(Into::into)
    }
}

impl ToSql<Text, Sqlite> for Sources {
    fn to_sql<'b>(&self, out: &mut serialize::Output<'b, '_, Sqlite>) -> serialize::Result {
        let json = serde_json::to_string(&self.0).unwrap_or_default();
        out.set_value(json);
        Ok(serialize::IsNull::No)
    }
}

#[derive(Queryable, Selectable, Clone, Debug, Serialize)]
#[diesel(table_name = movies)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Movie {
    pub id: i32,
    pub title: String,
    pub description: String,
    pub thumb: String,
    pub sources: Sources,
}
