use postgres;
use serde::{ Deserialize, Serialize };
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct Pipeline {
    pub pipeline_id : Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub created_by: Option<String>,
}

impl Pipeline {

    pub fn find(client: &mut postgres::Client, id: Uuid) -> Result<Option<Self>, postgres::error::Error> 
    {
        let results = &client.query(
            "SELECT pipeline_id, name, description, created_at, created_by \
             FROM test_framework.pipeline \
             WHERE pipeline_id = $1",
             &[&id],
        )?;

        if results.len() < 1 {
            return Ok(None)
        }

        let result_id: Uuid = results[0].get(0);

        Ok(Some(
            Pipeline {
                pipeline_id: Some((result_id).to_hyphenated().to_string()),
                name: results[0].get(1),
                description: results[0].get(2),
                created_at: results[0].get(3),
                created_by: results[0].get(4)
            }
        ))
    }

}
