use r2d2::{ ManageConnection, PooledConnection };
use serde::{ Deserialize, Serialize };

#[derive(Serialize, Deserialize)]
pub struct Pipeline {
    pub pipeline_id : Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub created_by: Option<String>,
}

impl Pipeline {

    pub fn find<M>(client: PooledConnection<M>, id: String) -> Result<Self, Error> 
        where T: r2d2::ManageConnection
    {
        let row = client.query(
            "SELECT pipeline_id, name, description, created_at, created_by \
             FROM test_framework.pipeline \
             WHERE pipeline_id = $1",
             &[&id],
        )?[0];

        Pipeline {
            pipeline_id: row.get(0),
            name: row.get(1),
            description: row.get(2),
            created_at: row.get(3),
            created_by: row.get(4)
        }
    }

}
