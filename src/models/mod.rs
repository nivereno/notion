pub mod block;
pub mod error;
pub mod paging;
pub mod properties;
pub mod search;
#[cfg(test)]
mod tests;
pub mod text;
pub mod users;

use crate::models::properties::{PropertyConfiguration, PropertyItem, PropertyValue};
use crate::models::text::RichText;
use crate::Error;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};

use crate::ids::{AsIdentifier, BlockId, DatabaseId, PageId};
use crate::models::block::{Block, CreateBlock, FileOrEmojiObject};
use crate::models::error::ErrorResponse;
use crate::models::paging::PagingCursor;
use crate::models::users::User;
pub use serde_json::value::Number;
pub use time::{Date, OffsetDateTime};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Copy, Clone)]
#[serde(rename_all = "snake_case")]
enum ObjectType {
    Database,
    List,
}

/// Represents a Notion Database
/// See <https://developers.notion.com/reference/database>
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct Database {
    /// Unique identifier for the database.
    pub id: DatabaseId,
    /// Date and time when this database was created.
    #[serde(with = "time::serde::iso8601")]
    pub created_time: OffsetDateTime,
    /// Date and time when this database was updated.
    #[serde(with = "time::serde::iso8601")]
    pub last_edited_time: OffsetDateTime,
    /// Name of the database as it appears in Notion.
    pub title: Vec<RichText>,
    /// Schema of properties for the database as they appear in Notion.
    //
    // key string
    // The name of the property as it appears in Notion.
    //
    // value object
    // A Property object.
    #[serde(serialize_with = "ordered_map")]
    pub properties: HashMap<String, PropertyConfiguration>,
}

impl Hash for Database {
    fn hash<H: Hasher>(
        &self,
        state: &mut H,
    ) {
        self.id.hash(state);
    }
}

impl AsIdentifier<DatabaseId> for Database {
    fn as_id(&self) -> &DatabaseId {
        &self.id
    }
}

impl Database {
    pub fn title_plain_text(&self) -> String {
        self.title
            .iter()
            .flat_map(|rich_text| rich_text.plain_text().chars())
            .collect()
    }
}

/// <https://developers.notion.com/reference/pagination#responses>
#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub struct ListResponse<T> {
    pub results: Vec<T>,
    pub next_cursor: Option<PagingCursor>,
    pub has_more: bool,
}

impl<T> ListResponse<T> {
    pub fn results(&self) -> &[T] {
        &self.results
    }
}

impl ListResponse<Object> {
    pub fn only_databases(self) -> ListResponse<Database> {
        let databases = self
            .results
            .into_iter()
            .filter_map(|object| match object {
                Object::Database { database } => Some(database),
                _ => None,
            })
            .collect();

        ListResponse {
            results: databases,
            has_more: self.has_more,
            next_cursor: self.next_cursor,
        }
    }

    pub(crate) fn expect_databases(self) -> Result<ListResponse<Database>, crate::Error> {
        let databases: Result<Vec<_>, _> = self
            .results
            .into_iter()
            .map(|object| match object {
                Object::Database { database } => Ok(database),
                response => Err(Error::UnexpectedResponse { response }),
            })
            .collect();

        Ok(ListResponse {
            results: databases?,
            has_more: self.has_more,
            next_cursor: self.next_cursor,
        })
    }

    pub(crate) fn expect_pages(self) -> Result<ListResponse<Page>, crate::Error> {
        let items: Result<Vec<_>, _> = self
            .results
            .into_iter()
            .map(|object| match object {
                Object::Page { page } => Ok(page),
                response => Err(Error::UnexpectedResponse { response }),
            })
            .collect();

        Ok(ListResponse {
            results: items?,
            has_more: self.has_more,
            next_cursor: self.next_cursor,
        })
    }

    pub(crate) fn expect_blocks(self) -> Result<ListResponse<Block>, crate::Error> {
        let items: Result<Vec<_>, _> = self
            .results
            .into_iter()
            .map(|object| match object {
                Object::Block { block } => Ok(block),
                response => Err(Error::UnexpectedResponse { response }),
            })
            .collect();

        Ok(ListResponse {
            results: items?,
            has_more: self.has_more,
            next_cursor: self.next_cursor,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
#[serde(tag = "object")]
#[serde(rename_all = "snake_case")]
pub enum PropertyResponse {
    List {
        results: ListResponse<PropertyValue>,
    },
    PropertyItem {
        property_item: PropertyValue,
    },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Parent {
    #[serde(rename = "database_id")]
    Database {
        database_id: DatabaseId,
    },
    #[serde(rename = "page_id")]
    Page {
        page_id: PageId,
    },
    #[serde(rename = "block_id")]
    Block {
        block_id: BlockId,
    },
    Workspace,
}

fn ordered_map<S, K: Ord + Serialize, V: Serialize>(
    value: &HashMap<K, V>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let ordered: BTreeMap<_, _> = value.iter().collect();
    ordered.serialize(serializer)
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct Properties {
    #[serde(flatten, serialize_with = "ordered_map")]
    pub properties: HashMap<String, PropertyValue>,
}

impl Properties {
    pub fn title(&self) -> Option<String> {
        self.properties.values().find_map(|p| match p {
            PropertyValue::Title { title, .. } => {
                Some(title.iter().map(|t| t.plain_text()).collect())
            }
            _ => None,
        })
    }
    pub fn title_object(&self) -> Option<&Vec<RichText>> {
        self.properties.values().find_map(|p| match p {
            PropertyValue::Title { title, .. } => Some(title),
            _ => None,
        })
    }
}

#[derive(Serialize, Debug, Eq, PartialEq, Clone)]
pub struct PageCreateRequest {
    pub parent: Parent,
    pub properties: Properties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<CreateBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<FileOrEmojiObject>,
}

#[derive(Serialize, Debug, Eq, PartialEq, Clone)]
pub struct UpdateBlockChildrenRequest {
    pub children: Vec<CreateBlock>,
}

#[derive(Serialize, Debug, Eq, PartialEq)]
pub struct PageUpdateRequest {
    pub properties: Properties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<FileOrEmojiObject>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct Page {
    pub id: PageId,
    /// Date and time when this page was created.
    #[serde(with = "time::serde::iso8601")]
    pub created_time: OffsetDateTime,
    /// Date and time when this page was updated.
    #[serde(with = "time::serde::iso8601")]
    pub last_edited_time: OffsetDateTime,
    /// The archived status of the page.
    pub archived: bool,
    pub properties: Properties,
    pub parent: Parent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<FileOrEmojiObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<Block>>,
}

impl Hash for Page {
    fn hash<H: Hasher>(
        &self,
        state: &mut H,
    ) {
        self.id.hash(state);
    }
}

impl Page {
    pub fn title(&self) -> Option<String> {
        self.properties.title()
    }
    pub fn title_object(&self) -> Option<&Vec<RichText>> {
        self.properties.title_object()
    }
}

impl AsIdentifier<PageId> for Page {
    fn as_id(&self) -> &PageId {
        &self.id
    }
}

#[derive(Eq, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "object")]
#[serde(rename_all = "snake_case")]
pub enum Object {
    Block {
        #[serde(flatten)]
        block: Block,
    },
    Database {
        #[serde(flatten)]
        database: Database,
    },
    Page {
        #[serde(flatten)]
        page: Page,
    },
    List {
        #[serde(flatten)]
        list: ListResponse<Object>,
    },
    User {
        #[serde(flatten)]
        user: User,
    },
    Error {
        #[serde(flatten)]
        error: ErrorResponse,
    },
    #[serde(rename = "property_item")]
    PropertyItem {
        #[serde(flatten)]
        property_item: PropertyItem,
    },
}

impl Object {
    pub fn is_database(&self) -> bool {
        matches!(self, Object::Database { .. })
    }
}
