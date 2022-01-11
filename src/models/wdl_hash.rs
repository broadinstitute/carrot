//! Contains structs and functions for doing operations on wdl_hash records.
//!
//! A wdl_hash is a record of the location of a WDL and a hash of its contents. Represented in the
//! database by the WDL_HASH table.

use crate::schema::wdl_hash;
use crate::schema::wdl_hash::dsl::*;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};

/// Mapping to a wdl_hash record as it exists in the WDL_HASH table in the database.
///
/// An instance of this struct will be returned by any queries for wdl_hashes.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct WdlHashData {
    pub location: String,
    pub hash: Vec<u8>,
    pub created_at: NaiveDateTime,
}

/// A new wdl_hash record to be inserted into the DB
///
/// location and data_to_hash are required fields, and created_at is populated automatically by the
/// DB
/// Before insertion, `data` is hashed using SHA-512 and the hash is what is actually
/// inserted into the DB
pub struct WdlDataToHash<'a> {
    pub location: String,
    pub data: &'a [u8],
}

/// The data we'll actually insert when creating a new WDL_HASH record
///
/// NewWdlHash is converted into this by hashing `data_to_hash` before insertion
#[derive(Deserialize, Serialize, Insertable)]
#[table_name = "wdl_hash"]
struct NewWdlHash {
    pub location: String,
    pub hash: Vec<u8>,
}

impl WdlHashData {
    /// Queries the DB for wdl_hash records for the specified data
    ///
    /// Queries the DB using `conn` to retrieve all rows with a hash matching the SHA-512 hash for
    /// `query_data`
    /// Returns a result containing either the retrieved wdl_hash mappings as a vector of
    /// WdlHashData instances or an error if the query fails for some reason
    pub fn find_by_data_to_hash(
        conn: &PgConnection,
        query_data: &[u8],
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Make a hash for the data
        let mut hasher: Sha512 = Sha512::new();
        hasher.update(query_data);
        let result: Vec<u8> = hasher.finalize().to_vec();
        wdl_hash.filter(hash.eq(&result)).load::<Self>(conn)
    }

    /// Inserts a new wdl_hash mapping into the DB
    ///
    /// Creates a new wdl_hash row in the DB using `conn` with the values specified in
    /// `params`
    /// Returns a result containing either the new wdl_hash mapping that was created or an
    /// error if the insert fails for some reason
    pub fn create(
        conn: &PgConnection,
        params: WdlDataToHash,
    ) -> Result<Self, diesel::result::Error> {
        // Hash data_to_hash
        let mut hasher: Sha512 = Sha512::new();
        hasher.update(&params.data);
        let result: Vec<u8> = hasher.finalize().to_vec();
        // Build the record we'll actually insert
        let new_wdl_hash = NewWdlHash {
            location: params.location,
            hash: result,
        };
        // Insert
        diesel::insert_into(wdl_hash)
            .values(&new_wdl_hash)
            .get_result(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::unit_test_util::get_test_db_connection;
    use sha2::{Digest, Sha512};

    fn insert_test_wdl_hash(conn: &PgConnection) -> WdlHashData {
        let new_wdl_hash = WdlDataToHash {
            location: String::from("/test/path/to/wdl.wdl"),
            data: b"Test data to hash",
        };

        WdlHashData::create(conn, new_wdl_hash).expect("Failed inserting test wdl_hash")
    }

    fn insert_test_wdl_hashes(conn: &PgConnection) -> Vec<WdlHashData> {
        let mut test_wdl_hashes: Vec<WdlHashData> = Vec::new();

        let new_wdl_hash = WdlDataToHash {
            location: String::from("/test/path/to/wdl.wdl"),
            data: b"Different test data to hash",
        };

        test_wdl_hashes
            .push(WdlHashData::create(conn, new_wdl_hash).expect("Failed inserting test wdl_hash"));

        let new_wdl_hash = WdlDataToHash {
            location: String::from("/different/path/to/wdl.wdl"),
            data: b"Different test data to hash",
        };

        test_wdl_hashes
            .push(WdlHashData::create(conn, new_wdl_hash).expect("Failed inserting test wdl_hash"));

        test_wdl_hashes
    }

    #[test]
    fn find_by_data_to_hash_exists() {
        let conn = get_test_db_connection();

        // Insert two records we're looking for and one we're not
        insert_test_wdl_hash(&conn);
        let test_wdl_hashes = insert_test_wdl_hashes(&conn);

        let found_wdl_hashes =
            WdlHashData::find_by_data_to_hash(&conn, b"Different test data to hash")
                .expect("Failed to retrieve test wdl_hash by hash");

        assert_eq!(found_wdl_hashes.len(), 2);
        assert!(found_wdl_hashes.contains(&test_wdl_hashes[0]));
        assert!(found_wdl_hashes.contains(&test_wdl_hashes[1]));
    }

    #[test]
    fn find_by_data_to_hash_not_exists() {
        let conn = get_test_db_connection();

        // Insert some values so we don't grab those
        insert_test_wdl_hashes(&conn);

        let empty_result =
            WdlHashData::find_by_data_to_hash(&conn, b"Random data we won't find").unwrap();

        assert_eq!(empty_result.len(), 0);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_wdl_hash = insert_test_wdl_hash(&conn);

        // Hash for comparison
        let mut hasher: Sha512 = Sha512::new();
        hasher.update(b"Test data to hash");
        let result: Vec<u8> = hasher.finalize().to_vec();

        assert_eq!(test_wdl_hash.location, "/test/path/to/wdl.wdl");
        assert_eq!(test_wdl_hash.hash, result);
    }

    #[test]
    fn create_failure_same_location_and_hash() {
        let conn = get_test_db_connection();

        let test_wdl_hash = insert_test_wdl_hash(&conn);

        let copy_wdl_hash = WdlDataToHash {
            location: test_wdl_hash.location,
            data: b"Test data to hash",
        };

        let new_wdl_hash = WdlHashData::create(&conn, copy_wdl_hash);

        assert!(matches!(
            new_wdl_hash,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ),)
        ));
    }
}
