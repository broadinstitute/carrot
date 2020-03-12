use chrono::{ DateTime, Utc };
use postgres;
use serde::{ Deserialize, Serialize };
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct Template {
    pub template_id : Option<Uuid>,
    pub pipeline_id : Option<Uuid>,
    pub name: String,
    pub test_wdl: String,
    pub eval_wdl: String,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<String>,
}

impl Template {

    pub fn find(client: &mut postgres::Client, id: Uuid) -> Result<Option<Self>, postgres::error::Error> 
    {
        let results = &client.query(
            "SELECT template_id, name, test_wdl, eval_wdl, created_at, created_by \
             FROM test_framework.template \
             WHERE template_id = $1",
             &[&id],
        )?;

        if results.len() < 1 {
            return Ok(None)
        }

        Ok(Some(
            Template {
                template_id: results[0].get(0),
                pipeline_id: results[0].get(1),
                name: results[0].get(2),
                test_wdl: results[0].get(3),
                eval_wdl: results[0].get(4),
                created_at: results[0].get(5),
                created_by: results[0].get(6)
            }
        ))
    }

}
