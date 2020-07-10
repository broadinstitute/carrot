//! Contains structs and functions for doing operations on subscriptions.
//!
//! A subscription is a mapping of a user email address to a specific entity in the database for
//! the purpose of being notified of events related to that entity. Represented in the database by
//! the SUBSCRIPTION table.

use crate::schema::subscription;
use crate::schema::subscription::dsl::*;
use crate::custom_sql_types::EntityTypeEnum;
use crate::util;
use uuid::Uuid;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

/// Mapping to a subscription as it exists in the SUBSCRIPTION table in the database.
///
/// An instance of this struct will be returned by any queries for subscriptions.
#[derive(Queryable, Serialize, Deserialize, PartialEq, Debug)]
pub struct SubscriptionData {
    pub subscription_id: Uuid,
    pub entity_type: EntityTypeEnum,
    pub entity_id: Uuid,
    pub email: String,
    pub created_at: NaiveDateTime,
}

/// Represents all possible parameters for a query of the SUBSCRIPTION table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(entity_type),desc(email),created_at
#[derive(Deserialize, Serialize)]
pub struct SubscriptionQuery {
    pub subscription_id: Option<Uuid>,
    pub entity_type: Option<EntityTypeEnum>,
    pub entity_id: Option<Uuid>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub email: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new subscription to be inserted into the DB
///
/// All fields are required
/// subscription_id and created_at are populated automatically by the DB
#[derive(Deserialize, Insertable, Serialize)]
#[table_name = "subscription"]
pub struct NewSubscription {
    pub entity_type: EntityTypeEnum,
    pub entity_id: Uuid,
    pub email: String,
}

/// Represents all possible parameters for a delete query of the SUBSCRIPTION table
///
/// All values are optional, so any combination can be used during a query.
pub struct SubscriptionDeleteParams {
    pub subscription_id: Option<Uuid>,
    pub entity_type: Option<EntityTypeEnum>,
    pub entity_id: Option<Uuid>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub email: Option<String>,
}

impl SubscriptionData {

    /// Queries the DB for a subscription with the specified id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a subscription_id value of `id`
    /// Returns a result containing either the retrieved subscription as a SubscriptionData
    /// instance or an error if the query fails for some reason or if no subscription is found
    /// matching the criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        subscription.filter(subscription_id.eq(id)).first::<Self>(conn)
    }

    /// Queries the DB for subscriptions matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve subscriptions matching the criteria in `params`
    /// Returns a result containing either a vector of the retrieved subscriptions as 
    /// SubscriptionData instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: SubscriptionQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = subscription.into_boxed();

        // Add filters for each of the params if they have values
        if let Some(param) = params.subscription_id {
            query = query.filter(subscription_id.eq(param));
        }
        if let Some(param) = params.entity_type {
            query = query.filter(entity_type.eq(param));
        }
        if let Some(param) = params.entity_id {
            query = query.filter(entity_id.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(created_at.gt(param));
        }
        if let Some(param) = params.email {
            query = query.filter(email.eq(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::parse_sort_string(&sort);
            for sort_clause in sort {
                match &*sort_clause.key {
                    "subscription_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(subscription_id.asc());
                        } else {
                            query = query.then_order_by(subscription_id.desc());
                        }
                    }
                    "entity_type" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(entity_type.asc());
                        } else {
                            query = query.then_order_by(entity_type.desc());
                        }
                    }
                    "entity_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(entity_id.asc());
                        } else {
                            query = query.then_order_by(entity_id.desc());
                        }
                    }
                    "created_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(created_at.asc());
                        } else {
                            query = query.then_order_by(created_at.desc());
                        }
                    }
                    "email" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(email.asc());
                        } else {
                            query = query.then_order_by(email.desc());
                        }
                    }
                    // Don't add to the order by clause if the sort key isn't recognized
                    &_ => {}
                }
            }
        }

        if let Some(param) = params.limit {
            query = query.limit(param);
        }
        if let Some(param) = params.offset {
            query = query.offset(param);
        }

        // Perform the query
        query.load::<Self>(conn)
    }

    /// Inserts a new subscription into the DB
    ///
    /// Creates a new subscription row in the DB using `conn` with the values specified in `params`
    /// Returns a result containing either the new subscription that was created or an error if
    /// the insert fails for some reason
    pub fn create(conn: &PgConnection, params: NewSubscription) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(subscription)
            .values(&params)
            .get_result(conn)
    }

    /// Deletes subscriptions from the DB
    ///
    /// Deletes subscriptions based on the params specified in `params`
    /// Returns either the number of subscriptions deleted, or an error if something goes wrong
    /// during the delete
    pub fn delete(conn: &PgConnection, params: SubscriptionDeleteParams) -> Result<usize, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = diesel::delete(subscription).into_boxed();

        // Add filters for each of the params if they have values
        if let Some(param) = params.subscription_id {
            query = query.filter(subscription_id.eq(param));
        }
        if let Some(param) = params.entity_type {
            query = query.filter(entity_type.eq(param));
        }
        if let Some(param) = params.entity_id {
            query = query.filter(entity_id.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(created_at.gt(param));
        }
        if let Some(param) = params.email {
            query = query.filter(email.eq(param));
        }

        query.execute(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::unit_test_util::*;
    use uuid::Uuid;

    fn insert_test_subscription(conn: &PgConnection) -> SubscriptionData {
        let new_subscription = NewSubscription {
            entity_type: EntityTypeEnum::Pipeline,
            entity_id: Uuid::new_v4(),
            email: String::from("Kevin@example.com"),
        };

        SubscriptionData::create(conn, new_subscription).expect("Failed inserting test subscription")
    }

    fn insert_test_subscriptions(conn: &PgConnection) -> Vec<SubscriptionData> {
        let mut subscriptions = Vec::new();

        let new_subscription = NewSubscription {
            entity_type: EntityTypeEnum::Pipeline,
            entity_id: Uuid::new_v4(),
            email: String::from("Kevin@example.com"),
        };

        subscriptions.push(
            SubscriptionData::create(conn, new_subscription).expect("Failed inserting test subscription"),
        );

        let new_subscription = NewSubscription {
            entity_type: EntityTypeEnum::Template,
            entity_id: Uuid::new_v4(),
            email: String::from("Kevin@example.com"),
        };

        subscriptions.push(
            SubscriptionData::create(conn, new_subscription).expect("Failed inserting test subscription"),
        );

        let new_subscription = NewSubscription {
            entity_type: EntityTypeEnum::Test,
            entity_id: Uuid::new_v4(),
            email: String::from("Kevin@example.com"),
        };

        subscriptions.push(
            SubscriptionData::create(conn, new_subscription).expect("Failed inserting test subscription"),
        );

        subscriptions
    }

    #[test]
    fn find_by_id_exists() {
        let conn = get_test_db_connection();

        let test_subscription = insert_test_subscription(&conn);

        let found_subscription = SubscriptionData::find_by_id(&conn, test_subscription.subscription_id)
            .expect("Failed to retrieve test subscription by id.");

        assert_eq!(found_subscription, test_subscription);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_subscription = SubscriptionData::find_by_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_subscription,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_subscription_id() {
        let conn = get_test_db_connection();

        let test_subscriptions = insert_test_subscriptions(&conn);

        let test_query = SubscriptionQuery {
            subscription_id: Some(test_subscriptions[0].subscription_id),
            entity_type: None,
            entity_id: None,
            created_before: None,
            created_after: None,
            email: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_subscriptions =
            SubscriptionData::find(&conn, test_query).expect("Failed to find subscriptions");

        assert_eq!(found_subscriptions.len(), 1);
        assert_eq!(found_subscriptions[0], test_subscriptions[0]);
    }

    #[test]
    fn find_with_entity_type() {
        let conn = get_test_db_connection();

        let test_subscriptions = insert_test_subscriptions(&conn);

        let test_query = SubscriptionQuery {
            subscription_id: None,
            entity_type: Some(EntityTypeEnum::Test),
            entity_id: None,
            created_before: None,
            created_after: None,
            email: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_subscriptions =
            SubscriptionData::find(&conn, test_query).expect("Failed to find subscriptions");

        assert_eq!(found_subscriptions.len(), 1);
        assert_eq!(found_subscriptions[0], test_subscriptions[2]);
    }

    #[test]
    fn find_with_entity_id() {
        let conn = get_test_db_connection();

        let test_subscriptions = insert_test_subscriptions(&conn);

        let test_query = SubscriptionQuery {
            subscription_id: None,
            entity_type: None,
            entity_id: Some(test_subscriptions[0].entity_id.clone()),
            created_before: None,
            created_after: None,
            email: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_subscriptions =
            SubscriptionData::find(&conn, test_query).expect("Failed to find subscriptions");

        assert_eq!(found_subscriptions.len(), 1);
        assert_eq!(found_subscriptions[0], test_subscriptions[0]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_subscriptions = insert_test_subscriptions(&conn);

        let test_query = SubscriptionQuery {
            subscription_id: None,
            entity_type: None,
            entity_id: None,
            created_before: None,
            created_after: None,
            email: Some(String::from("Kevin@example.com")),
            sort: Some(String::from("desc(entity_type)")),
            limit: Some(2),
            offset: None,
        };

        let found_subscriptions =
            SubscriptionData::find(&conn, test_query).expect("Failed to find subscriptions");

        assert_eq!(found_subscriptions.len(), 2);
        assert_eq!(found_subscriptions[0], test_subscriptions[2]);
        assert_eq!(found_subscriptions[1], test_subscriptions[1]);

        let test_query = SubscriptionQuery {
            subscription_id: None,
            entity_type: None,
            entity_id: None,
            created_before: None,
            created_after: None,
            email: Some(String::from("Kevin@example.com")),
            sort: Some(String::from("desc(entity_type)")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_subscriptions =
            SubscriptionData::find(&conn, test_query).expect("Failed to find subscriptions");

        assert_eq!(found_subscriptions.len(), 1);
        assert_eq!(found_subscriptions[0], test_subscriptions[0]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_subscriptions(&conn);

        let test_query = SubscriptionQuery {
            subscription_id: None,
            entity_type: None,
            entity_id: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            email: Some(String::from("Kevin@example.com")),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_subscriptions =
            SubscriptionData::find(&conn, test_query).expect("Failed to find subscriptions");

        assert_eq!(found_subscriptions.len(), 0);

        let test_query = SubscriptionQuery {
            subscription_id: None,
            entity_type: None,
            entity_id: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            email: Some(String::from("Kevin@example.com")),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_subscriptions =
            SubscriptionData::find(&conn, test_query).expect("Failed to find subscriptions");

        assert_eq!(found_subscriptions.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_subscription = insert_test_subscription(&conn);

        assert_eq!(test_subscription.entity_type, EntityTypeEnum::Pipeline);
        assert_eq!(
            test_subscription
                .email,
            "Kevin@example.com"
        );
    }

    #[test]
    fn delete_success() {
        let conn = get_test_db_connection();

        let test_subscription = insert_test_subscription(&conn);

        // Make sure we can find it
        SubscriptionData::find_by_id(&conn, test_subscription.subscription_id.clone())
            .expect("Could not find test_subscription after insert");

        // Delete it
        let delete_params = SubscriptionDeleteParams {
            subscription_id: Some(test_subscription.subscription_id.clone()),
            entity_type: None,
            entity_id: None,
            created_before: None,
            created_after: None,
            email: None,
        };
        let delete_count = SubscriptionData::delete(&conn, delete_params)
            .expect("Error during delete");

        assert_eq!(delete_count, 1);

        // Make sure we can't find it now
        let find_after_delete = SubscriptionData::find_by_id(&conn, test_subscription.subscription_id);

        assert!(matches!(find_after_delete, Err(diesel::result::Error::NotFound)));
    }
}

