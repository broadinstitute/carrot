---
layout: default
title: run
description: "Commands for searching, creating, and updating runs"
nav_order: 6
parent: Commands
---

# Run
{: .no_toc}

## Table of contents
{: .no_toc .text-delta}

* TOC
{:toc}

---

## Description

A run is a specific instance of a test that includes the docker container of the software to run and any other parameters specific to a software release. This run is specified and then executed by CARROT producing results which then can be interpreted.

## Commands

### Create report
```shell
$ carrot_cli run create_report --help
Usage: carrot_cli run create_report [OPTIONS] ID REPORT_ID

  Start a job to generate a filled report using data from the run specified
  by ID with the report specified by REPORT_ID

Options:
  --created_by TEXT  Email of the creator of the mapping
  --delete_failed    If set, and there is a failed record for this run with
                     this report, will overwrite that record

  --help             Show this message and exit.
```

### Delete
```shell
$ carrot_cli run delete --help
Usage: carrot_cli run delete [OPTIONS] ID

  Delete a run by its ID, if the run has a failed status

Options:
  --help  Show this message and exit.
```

### Delete report by ids
```shell
$ carrot_cli run delete_report_by_ids --help
Usage: carrot_cli run delete_report_by_ids [OPTIONS] ID REPORT_ID

  Delete the report record for the run specified by ID to the report
  specified by REPORT_ID

Options:
  --help  Show this message and exit.
```

### Find by id
```shell
$ carrot_cli run find_by_id --help
Usage: carrot_cli run find_by_id [OPTIONS] ID

  Retrieve a run by its ID

Options:
  --help  Show this message and exit.
```

### Find report by ids
```shell
$ carrot_cli run find_report_by_ids --help
Usage: carrot_cli run find_report_by_ids [OPTIONS] ID REPORT_ID

  Retrieve the report record for the run specified by ID and the report
  specified by REPORT_ID

Options:
  --help  Show this message and exit.
```

### Find reports
```shell
$ carrot_cli run find_reports --help
Usage: carrot_cli run find_reports [OPTIONS] ID

  Retrieve the report record from the run specified by ID for the report
  specified by REPORT_ID

Options:
  --report_id TEXT        The id of the report
  --status TEXT           The status of the job generating the report
  --cromwell_job_id TEXT  The id for the cromwell job for generating the
                          filled report

  --results TEXT          A json file containing the results of the report job
  --created_before TEXT   Upper bound for report record's created_at value, in
                          the format YYYY-MM-DDThh:mm:ss.ssssss

  --created_after TEXT    Lower bound for report record's created_at value, in
                          the format YYYY-MM-DDThh:mm:ss.ssssss

  --created_by TEXT       Email of the creator of the report record, case
                          sensitive

  --finished_before TEXT  Upper bound for report record's finished_at value,
                          in the format YYYY-MM-DDThh:mm:ss.ssssss

  --finished_after TEXT   Lower bound for report record's finished_at value,
                          in the format YYYY-MM-DDThh:mm:ss.ssssss

  --sort TEXT             A comma-separated list of sort keys, enclosed in
                          asc() for ascending or desc() for descending.  Ex.
                          asc(input_map),desc(report_id)

  --limit INTEGER         The maximum number of map records to return
                          [default: 20]

  --offset INTEGER        The offset to start at within the list of records to
                          return.  Ex. Sorting by asc(created_at) with
                          offset=1 would return records sorted by when they
                          were created starting from the second record to be
                          created  [default: 0]

  --help                  Show this message and exit.
```