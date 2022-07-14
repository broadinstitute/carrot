//! Defines functions for writing run data to CSV files

use crate::models::run::RunWithResultsAndErrorsData;
use csv::Writer;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::Formatter;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;
use zip::write::FileOptions;
use zip::ZipWriter;

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Csv(csv::Error),
    Zip(zip::result::ZipError),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::IO(e) => write!(f, "run_csv Error IO {}", e),
            Error::Csv(e) => write!(f, "run_csv Error CSV {}", e),
            Error::Zip(e) => write!(f, "run_csv Error Zip {:?}", e),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IO(e)
    }
}

impl From<csv::Error> for Error {
    fn from(e: csv::Error) -> Error {
        Error::Csv(e)
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(e: zip::result::ZipError) -> Error {
        Error::Zip(e)
    }
}

/// Enum for type of csv file (of the types that are processed using build_rows)
#[derive(Copy, Clone)]
enum CSVContentsType {
    TestInput,
    EvalInput,
    TestOptions,
    EvalOptions,
    Results,
}

/// Creates a TempDir, writes data for `runs` into CSV files in it, along with a zip of all of them,
/// and returns the TempDir. Returns an Error if anything goes wrong.
///
/// The returned TempDir contains 7 files:
/// - metadata.csv - contains a row for each run with metadata for that row (rund_id, name, test_id,
///                  status, cromwell ids, created_at, created_by, finished_at, and errors)
/// - test_inputs.csv - contains a row for each run with its run id and the contents of the
///                     test_input json for that run, with input names as column headers
/// - eval_inputs.csv - contains a row for each run with its run id and the contents of the
///                     eval_input json for that run, with input names as column headers
/// - test_options.csv - contains a row for each run with its run id and the contents of the
///                      test_options json for that run, with option names as column headers
/// - eval_options.csv - contains a row for each run with its run id and the contents of the
///                      eval_options json for that run, with option names as column headers
/// - results.csv - contains a row for each run with its run id and the contents of the results
///                 json for that run, with the result names as column headers
/// - run_csv.zip - a zip containing the other 6 files
///
/// Zipping functionality is partially adapted from
/// https://github.com/zip-rs/zip/blob/5d0f198124946b7be4e5969719a7f29f363118cd/examples/write_dir.rs
pub fn write_run_data_to_csvs_and_zip_in_temp_dir(
    runs: &[RunWithResultsAndErrorsData],
) -> Result<TempDir, Error> {
    let csv_dir: TempDir = write_run_data_to_csvs_in_temp_dir(runs)?;
    // Create a file to write the data to
    let mut zip_file_path: PathBuf = PathBuf::from(csv_dir.path());
    zip_file_path.push("run_csvs.zip");
    let zip_file: File = File::create(&zip_file_path)?;
    // Create a writer and write each file to the zip
    let mut zip_writer: ZipWriter<File> = ZipWriter::new(zip_file);
    let mut buffer: Vec<u8> = Vec::new();
    for csv_filename in [
        "metadata.csv",
        "test_inputs.csv",
        "eval_inputs.csv",
        "test_options.csv",
        "eval_options.csv",
        "results.csv",
    ] {
        // Tell the zip writer we're starting a new file
        zip_writer.start_file(csv_filename, FileOptions::default())?;
        // Open the file we're compressing
        let mut file_path: PathBuf = PathBuf::from(csv_dir.path());
        file_path.push(csv_filename);
        let mut csv_file: File = File::open(file_path)?;
        csv_file.read_to_end(&mut buffer)?;
        zip_writer.write_all(&*buffer)?;
        buffer.clear();
    }
    // Close the zip writer
    zip_writer.finish()?;

    Ok(csv_dir)
}

/// Creates a TempDir, writes data for `runs` into CSV files in it, and returns the TempDir. Returns
/// an Error if anything goes wrong.
///
/// The returned TempDir contains 6 files:
/// - metadata.csv - contains a row for each run with metadata for that row (rund_id, name, test_id,
///                  status, cromwell ids, created_at, created_by, finished_at, and errors)
/// - test_inputs.csv - contains a row for each run with its run id and the contents of the
///                     test_input json for that run, with input names as column headers
/// - eval_inputs.csv - contains a row for each run with its run id and the contents of the
///                     eval_input json for that run, with input names as column headers
/// - test_options.csv - contains a row for each run with its run id and the contents of the
///                      test_options json for that run, with option names as column headers
/// - eval_options.csv - contains a row for each run with its run id and the contents of the
///                      eval_options json for that run, with option names as column headers
/// - results.csv - contains a row for each run with its run id and the contents of the results
///                 json for that run, with the result names as column headers
pub fn write_run_data_to_csvs_in_temp_dir(
    runs: &[RunWithResultsAndErrorsData],
) -> Result<TempDir, Error> {
    // Create the tempdir we'll put the files in
    let csv_dir = TempDir::new()?;
    // For each file, build the rows and write them to the file
    let metadata_rows: Vec<Vec<String>> = build_metadata_rows(runs);
    let mut metadata_writer: Writer<File> =
        init_writer_from_dir_and_name(csv_dir.path(), "metadata.csv")?;
    for row in metadata_rows {
        metadata_writer.write_record(&row)?;
    }
    metadata_writer.flush()?;
    // Loop for the other files because they all use the same function
    for (csv_content_type, filename) in [
        (CSVContentsType::TestInput, "test_inputs.csv"),
        (CSVContentsType::EvalInput, "eval_inputs.csv"),
        (CSVContentsType::TestOptions, "test_options.csv"),
        (CSVContentsType::EvalOptions, "eval_options.csv"),
        (CSVContentsType::Results, "results.csv"),
    ] {
        let rows: Vec<Vec<String>> = build_rows(runs, csv_content_type);
        let mut writer: Writer<File> = init_writer_from_dir_and_name(csv_dir.path(), filename)?;
        for row in rows {
            writer.write_record(&row)?;
        }
        writer.flush()?;
    }
    // Return the TempDir that now contains a bunch of CSVs
    Ok(csv_dir)
}

/// Builds and returns a vec of rows to write to a metadata csv file for `runs`
fn build_metadata_rows(runs: &[RunWithResultsAndErrorsData]) -> Vec<Vec<String>> {
    // Start by creating header row
    let header_row: Vec<String> = vec![
        String::from("run_id"),
        String::from("test_id"),
        String::from("name"),
        String::from("status"),
        String::from("test_cromwell_job_id"),
        String::from("eval_cromwell_job_id"),
        String::from("created_at"),
        String::from("created_by"),
        String::from("finished_at"),
        String::from("errors"),
    ];
    // Create our row vec and add the header
    let mut rows: Vec<Vec<String>> = vec![header_row];
    // Loop through the runs and add rows for each
    for run in runs {
        rows.push(vec![
            run.run_id.to_string(),
            run.test_id.to_string(),
            run.name.clone(),
            run.status.to_string(),
            match &run.test_cromwell_job_id {
                Some(t) => t.to_string(),
                None => String::new(),
            },
            match &run.eval_cromwell_job_id {
                Some(e) => e.to_string(),
                None => String::from(""),
            },
            run.created_at.to_string(),
            match &run.created_by {
                Some(c) => c.to_string(),
                None => String::from(""),
            },
            match &run.finished_at {
                Some(f) => f.to_string(),
                None => String::from(""),
            },
            match &run.errors {
                Some(e) => e.to_string(),
                None => String::from(""),
            },
        ]);
    }
    // Return our list of rows
    rows
}

/// Builds and returns a vec of rows containing a header followed by one row for each run in `runs`
/// with its run_id and values corresponding to `csv_contents_type` (e.g. if `csv_contents_type` is
/// CSVContentsType::TestInputs, the row will contain the values in the run's test_input field)
fn build_rows(
    runs: &[RunWithResultsAndErrorsData],
    csv_contents_type: CSVContentsType,
) -> Vec<Vec<String>> {
    // First, we'll build our header row
    let mut headers: HashSet<String> = HashSet::new();
    // We'll keep track of the processed maps for each run so we don't have to process them more
    // than once
    let mut processed_maps: HashMap<Uuid, HashMap<String, String>> = HashMap::new();
    // Loop through runs and add each new key we find to headers
    for run in runs {
        // Process the map (corresponding to map_type) for this run to get the values as strings in
        // the format in which we want to print them
        let processed_map: HashMap<String, String> = match csv_contents_type {
            CSVContentsType::TestInput => get_object_as_map_with_string_values(&run.test_input),
            CSVContentsType::EvalInput => get_object_as_map_with_string_values(&run.eval_input),
            CSVContentsType::TestOptions => match &run.test_options {
                Some(test_options) => get_object_as_map_with_string_values(test_options),
                None => HashMap::new(),
            },
            CSVContentsType::EvalOptions => match &run.eval_options {
                Some(eval_options) => get_object_as_map_with_string_values(eval_options),
                None => HashMap::new(),
            },
            CSVContentsType::Results => match &run.results {
                Some(results) => get_object_as_map_with_string_values(results),
                None => HashMap::new(),
            },
        };
        // Loop through its keys and add any new ones to headers
        for key in processed_map.keys() {
            if !headers.contains(key) {
                headers.insert(key.clone());
            }
        }
        // Make sure we hold on to processed_map for later
        processed_maps.insert(run.run_id, processed_map);
    }
    // Make a header row from headers
    let mut header_row: Vec<String> = vec![String::from("run_id")];
    // Collecting headers into their own vec first so they're sorted before we append to header_row
    let mut rest_of_headers: Vec<String> = headers.into_iter().collect();
    rest_of_headers.sort();
    header_row.extend(rest_of_headers);
    // Make our rows vector, starting with the header row
    let mut rows: Vec<Vec<String>> = vec![header_row];
    // Make a lookup map for header indices
    let header_index_map: HashMap<String, usize> = rows[0]
        .iter()
        .enumerate()
        .map(|(index, header)| (header.to_string(), index))
        .collect();
    // Now loop through runs and fill in data for all of them
    for run in runs {
        // Make a vec of empty strings to start
        let mut row: Vec<String> = vec![String::from(""); rows[0].len()];
        // Add the run_id at the start
        row[0] = run.run_id.to_string();
        let val_map: &HashMap<String, String> = processed_maps
            .get(&run.run_id)
            .unwrap_or_else(|| panic!("Failed to get map for run.  This should not happen."));
        // Loop through val_map and add those
        for (key, val) in val_map {
            // Get the index for this key
            let index: usize = *header_index_map.get(key as &str).unwrap_or_else(|| {
                panic!(
                    "Failed to get index for {} from index map. This should not happen.",
                    key
                )
            });
            // Insert the value into the row
            row[index] = val.clone();
        }
        rows.push(row);
    }
    rows
}

/// Assumes `value` is an object (or null) and returns a map of the keys to the values as strings
/// (or an empty map if null).
///
/// # Panics
/// Panics if `value` is anything other than Value::Object or Value::Null
fn get_object_as_map_with_string_values(value: &Value) -> HashMap<String, String> {
    match value {
        Value::Object(object_map) => {
            // Make a new map to return
            let mut processed_map: HashMap<String, String> = HashMap::new();
            // Loop through key,val pairs in object_map and add them to processed map
            for (key, val) in object_map.iter() {
                processed_map.insert(key.clone(), get_string_for_json_value(val));
            }
            processed_map
        }
        // Just return an empty map if it's null
        Value::Null => HashMap::new(),
        // Otherwise, panic
        _ => panic!(
            "Attempted to parse json val as object for building csv.  Failed with value: {}",
            value
        ),
    }
}

/// Converts json `value` to string.  Defaults to the Value to_string method, but extracts the actual
/// string value for if `value` is a Value::String so it isn't enclosed in quotes
fn get_string_for_json_value(value: &Value) -> String {
    match value {
        // If it's a string, get the actual string value
        Value::String(s) => s.clone(),
        // Any other type, just get the string representation of the json value
        _ => value.to_string(),
    }
}

/// Convenience function for creating a csv writer for a file from the directory for the file and
/// the name to use for it.  This function is just here to make write_run_data_to_csvs_in_temp_dir
/// a little more readable
fn init_writer_from_dir_and_name(
    dir_path: &Path,
    csv_name: &str,
) -> Result<Writer<File>, csv::Error> {
    let mut csv_file_path: PathBuf = PathBuf::from(dir_path);
    csv_file_path.push(csv_name);
    Writer::from_path(csv_file_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_sql_types::RunStatusEnum;
    use crate::models::run::RunWithResultsAndErrorsData;
    use chrono::{NaiveDateTime, Utc};
    use serde_json::json;
    use std::fs::read_to_string;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn create_test_runs() -> Vec<RunWithResultsAndErrorsData> {
        vec![
            RunWithResultsAndErrorsData {
                run_id: Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap(),
                test_id: Uuid::parse_str("8dd6d043-e16c-406c-8828-ce7ec2143e7f").unwrap(),
                name: "Test Run 1".to_string(),
                status: RunStatusEnum::Succeeded,
                test_input: json!({
                    "test_workflow.string": "hello",
                    "test_workflow.number": 4,
                    "test_workflow.array": [1,2,3]
                }),
                test_options: Some(json!({
                    "docker": "ubuntu:latest"
                })),
                eval_input: json!({
                    "eval_workflow.string": "goodbye",
                    "eval_workflow.file": "test_output:test_workflow.output_file"
                }),
                eval_options: None,
                test_cromwell_job_id: Some(String::from("0d0e02d5-070f-4240-a385-6c276bc07dd3")),
                eval_cromwell_job_id: Some(String::from("18a83622-3578-4266-b1b5-43faac6a00f1")),
                created_at: "2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap(),
                created_by: Some(String::from("test@example.com")),
                finished_at: Some("2099-01-01T12:00:00".parse::<NaiveDateTime>().unwrap()),
                results: Some(json!({
                    "output_file": "gs://example/path/to/file.mp4",
                    "output_number": 7
                })),
                errors: None,
            },
            RunWithResultsAndErrorsData {
                run_id: Uuid::parse_str("4f27abe7-2cfa-42c3-acc2-ce5344b0e471").unwrap(),
                test_id: Uuid::parse_str("8dd6d043-e16c-406c-8828-ce7ec2143e7f").unwrap(),
                name: "Test Run 2".to_string(),
                status: RunStatusEnum::Succeeded,
                test_input: json!({
                    "test_workflow.string": "bonjour",
                    "test_workflow.number": 4,
                    "test_workflow.array": [1,2,5,8]
                }),
                test_options: Some(json!({
                    "docker": "ubuntu:18.04"
                })),
                eval_input: json!({
                    "eval_workflow.string": "au revoir",
                    "eval_workflow.file": "test_output:test_workflow.output_file"
                }),
                eval_options: None,
                test_cromwell_job_id: Some(String::from("f34b7d2a-d0f8-49d6-aace-3b82ccb4c084")),
                eval_cromwell_job_id: Some(String::from("4dc770b8-df72-4cb3-9599-f758cefb5788")),
                created_at: "2099-01-01T23:00:00".parse::<NaiveDateTime>().unwrap(),
                created_by: Some(String::from("test@example.com")),
                finished_at: Some("2099-01-02T00:00:00".parse::<NaiveDateTime>().unwrap()),
                results: Some(json!({
                    "output_file": "gs://example/path/to/different/file.mp4",
                    "output_number": 5
                })),
                errors: None,
            },
            RunWithResultsAndErrorsData {
                run_id: Uuid::parse_str("4b2d5150-7d86-4a11-888c-b78dc030ba4f").unwrap(),
                test_id: Uuid::parse_str("8dd6d043-e16c-406c-8828-ce7ec2143e7f").unwrap(),
                name: "Test Run 3".to_string(),
                status: RunStatusEnum::Succeeded,
                test_input: json!({
                    "test_workflow.string": "hello",
                    "test_workflow.number": 6,
                    "test_workflow.array": [1,2,5,9]
                }),
                test_options: None,
                eval_input: json!({
                    "eval_workflow.string": "goodbye",
                    "eval_workflow.file": "test_output:test_workflow.output_file",
                    "eval_workflow.optional_thing": "yes"
                }),
                eval_options: Some(json!({
                    "continueOnReturnCode": true
                })),
                test_cromwell_job_id: Some(String::from("50cb6937-20d7-4bdc-be91-e09f9f861288")),
                eval_cromwell_job_id: Some(String::from("8eedefc2-5b41-4967-b6eb-c7034d9c8f15")),
                created_at: "2099-01-02T12:59:00".parse::<NaiveDateTime>().unwrap(),
                created_by: None,
                finished_at: Some("2099-01-03T00:00:00".parse::<NaiveDateTime>().unwrap()),
                results: Some(json!({
                    "output_file": "gs://example/path/to/different/file.mp4",
                    "output_number": 5,
                    "new_result": "yes"
                })),
                errors: None,
            },
            RunWithResultsAndErrorsData {
                run_id: Uuid::parse_str("cf4b5cb2-976e-4f9c-91ba-2b421d61199b").unwrap(),
                test_id: Uuid::parse_str("8dd6d043-e16c-406c-8828-ce7ec2143e7f").unwrap(),
                name: "Test Run 4".to_string(),
                status: RunStatusEnum::CarrotFailed,
                test_input: json!({
                    "test_workflow.string": "hello",
                    "test_workflow.number": 6,
                }),
                test_options: None,
                eval_input: json!({
                    "eval_workflow.string": "goodbye",
                    "eval_workflow.file": "test_output:test_workflow.output_file"
                }),
                eval_options: None,
                test_cromwell_job_id: None,
                eval_cromwell_job_id: None,
                created_at: "2099-01-03T12:59:00".parse::<NaiveDateTime>().unwrap(),
                created_by: None,
                finished_at: None,
                results: None,
                errors: Some(json!([
                    "Carrot failed to start the run because of a weird error"
                ])),
            },
        ]
    }

    #[test]
    fn write_run_data_to_csvs_in_temp_dir_with_zip_success() {
        // Get test runs to use
        let runs: Vec<RunWithResultsAndErrorsData> = create_test_runs();
        // Generate the csv files
        let csv_dir: TempDir = write_run_data_to_csvs_and_zip_in_temp_dir(&runs).unwrap();

        // Check that each file matches what we expect
        let test_file_contents = read_to_string(format!(
            "{}/{}",
            csv_dir.path().to_string_lossy(),
            "metadata.csv"
        ))
        .unwrap();
        let truth_file_contents = read_to_string("testdata/util/run_csv/metadata.csv").unwrap();
        assert_eq!(test_file_contents, truth_file_contents);

        let test_file_contents = read_to_string(format!(
            "{}/{}",
            csv_dir.path().to_string_lossy(),
            "test_inputs.csv"
        ))
        .unwrap();
        let truth_file_contents = read_to_string("testdata/util/run_csv/test_inputs.csv").unwrap();
        assert_eq!(test_file_contents, truth_file_contents);

        let test_file_contents = read_to_string(format!(
            "{}/{}",
            csv_dir.path().to_string_lossy(),
            "eval_inputs.csv"
        ))
        .unwrap();
        let truth_file_contents = read_to_string("testdata/util/run_csv/eval_inputs.csv").unwrap();
        assert_eq!(test_file_contents, truth_file_contents);

        let test_file_contents = read_to_string(format!(
            "{}/{}",
            csv_dir.path().to_string_lossy(),
            "results.csv"
        ))
        .unwrap();
        let truth_file_contents = read_to_string("testdata/util/run_csv/results.csv").unwrap();
        assert_eq!(test_file_contents, truth_file_contents);

        let test_file_contents = read_to_string(format!(
            "{}/{}",
            csv_dir.path().to_string_lossy(),
            "test_options.csv"
        ))
        .unwrap();
        let truth_file_contents = read_to_string("testdata/util/run_csv/test_options.csv").unwrap();
        assert_eq!(test_file_contents, truth_file_contents);

        let test_file_contents = read_to_string(format!(
            "{}/{}",
            csv_dir.path().to_string_lossy(),
            "eval_options.csv"
        ))
        .unwrap();
        let truth_file_contents = read_to_string("testdata/util/run_csv/eval_options.csv").unwrap();
        assert_eq!(test_file_contents, truth_file_contents);
    }

    #[test]
    fn write_run_data_to_csvs_in_temp_dir_success() {
        // Get test runs to use
        let runs: Vec<RunWithResultsAndErrorsData> = create_test_runs();
        // Generate the csv files
        let csv_dir: TempDir = write_run_data_to_csvs_in_temp_dir(&runs).unwrap();

        // Check that each file matches what we expect
        let test_file_contents = read_to_string(format!(
            "{}/{}",
            csv_dir.path().to_string_lossy(),
            "metadata.csv"
        ))
        .unwrap();
        let truth_file_contents = read_to_string("testdata/util/run_csv/metadata.csv").unwrap();
        assert_eq!(test_file_contents, truth_file_contents);

        let test_file_contents = read_to_string(format!(
            "{}/{}",
            csv_dir.path().to_string_lossy(),
            "test_inputs.csv"
        ))
        .unwrap();
        let truth_file_contents = read_to_string("testdata/util/run_csv/test_inputs.csv").unwrap();
        assert_eq!(test_file_contents, truth_file_contents);

        let test_file_contents = read_to_string(format!(
            "{}/{}",
            csv_dir.path().to_string_lossy(),
            "eval_inputs.csv"
        ))
        .unwrap();
        let truth_file_contents = read_to_string("testdata/util/run_csv/eval_inputs.csv").unwrap();
        assert_eq!(test_file_contents, truth_file_contents);

        let test_file_contents = read_to_string(format!(
            "{}/{}",
            csv_dir.path().to_string_lossy(),
            "results.csv"
        ))
        .unwrap();
        let truth_file_contents = read_to_string("testdata/util/run_csv/results.csv").unwrap();
        assert_eq!(test_file_contents, truth_file_contents);

        let test_file_contents = read_to_string(format!(
            "{}/{}",
            csv_dir.path().to_string_lossy(),
            "test_options.csv"
        ))
        .unwrap();
        let truth_file_contents = read_to_string("testdata/util/run_csv/test_options.csv").unwrap();
        assert_eq!(test_file_contents, truth_file_contents);

        let test_file_contents = read_to_string(format!(
            "{}/{}",
            csv_dir.path().to_string_lossy(),
            "eval_options.csv"
        ))
        .unwrap();
        let truth_file_contents = read_to_string("testdata/util/run_csv/eval_options.csv").unwrap();
        assert_eq!(test_file_contents, truth_file_contents);
    }
}
