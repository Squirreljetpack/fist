use cba::bath::{bytes_to_os_string, os_str_to_bytes};
use sqlx::{
    Database, Decode, Encode, Sqlite, Type,
    encode::IsNull,
    error::BoxDynError,
    prelude::FromRow,
    sqlite::{SqliteTypeInfo, SqliteValueRef},
};
use std::ffi::OsString;

use super::Epoch;
use crate::abspath::{AbsPath, OsStringWrapper};

#[derive(Debug, Clone, FromRow)]
pub struct Entry {
    pub name: String,
    pub path: AbsPath,
    pub alias: String,
    pub cmd: OsStringWrapper,
    // Initialized to 0 in `Entry::new`; set to the current atime (tick or
    // wall-clock) by `set_entry` when the entry is first pushed to the db.
    pub atime: Epoch,
    pub count: i32, // should be non-negative but currently leaky
    pub score: f64,
}

impl Entry {
    pub fn new(
        name: impl Into<String>,
        path: AbsPath,
    ) -> Self {
        Self {
            name: name.into(),
            path,
            atime: 0,
            alias: String::new(),
            cmd: OsStringWrapper::default(),
            count: 1,
            score: 1.0,
        }
    }

    pub fn cmd(
        mut self,
        cmd: OsString,
    ) -> Self {
        self.cmd = cmd.into();
        self
    }
}

impl Type<Sqlite> for AbsPath {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <Vec<u8> as Type<Sqlite>>::type_info()
    }
}

impl<'r> Decode<'r, Sqlite> for AbsPath {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let bytes = <Vec<u8> as Decode<Sqlite>>::decode(value)?;
        Ok(AbsPath::new_unchecked(bytes_to_os_string(bytes)))
    }
}

impl<'q> Encode<'q, Sqlite> for AbsPath {
    fn encode_by_ref(
        &self,
        buf: &mut <Sqlite as Database>::ArgumentBuffer,
    ) -> Result<IsNull, BoxDynError> {
        let bytes = os_str_to_bytes(self.as_os_str());
        <Vec<u8> as Encode<Sqlite>>::encode(bytes.into_owned(), buf)
    }
}

impl Type<Sqlite> for OsStringWrapper {
    fn type_info() -> SqliteTypeInfo {
        <Vec<u8> as Type<Sqlite>>::type_info()
    }
}

// Decode impl
impl<'r> Decode<'r, Sqlite> for OsStringWrapper {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let bytes = <Vec<u8> as Decode<Sqlite>>::decode(value)?;
        Ok(OsStringWrapper::from(bytes_to_os_string(bytes)))
    }
}

// Encode impl
impl<'q> Encode<'q, Sqlite> for OsStringWrapper {
    fn encode_by_ref(
        &self,
        buf: &mut <Sqlite as Database>::ArgumentBuffer,
    ) -> Result<IsNull, BoxDynError> {
        let bytes = os_str_to_bytes(self.as_os_str());
        <Vec<u8> as Encode<Sqlite>>::encode(bytes.into_owned(), buf)
    }
}
