//! Contains definitions of SQL functions using the `diesel::sql_function!` macro to enable their
//! use in diesel queries
//!
//! A lot of SQL functions built into Postgres are not explicitly defined in diesel, so we'll add
//! them here, along with any custom functions we ever define, so they can be used in queries in
//! the models submodules

use diesel::sql_types::*;

sql_function!(fn lower(x: Text) -> Text);