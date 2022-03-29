---
layout: default
title: template
description: "Commands for searching, creating, and updating templates"
nav_order: 3
parent: Commands
---

# Template
{: .no_toc}

## Table of contents
{: .no_toc .text-delta}

* TOC
{:toc}

---

## Description

Each CARROT pipeline can have one or more templates associated with it. A template defines a repeatable test and evaluation to be performed on the associated pipeline. This test and evaluation are defined in WDL files and do not have input values associated with them - rather they define a specific method to test and evaluate the pipeline which can be performed for multiple inputs. This allows the template to be run multiple times with multiple inputs, but with the same evaluation method.

## Commands

### Create
```shell
$ carrot_cli template create --help
Usage: carrot_cli template create [OPTIONS]

  Create template with the specified parameters

Options:
  --pipeline_id TEXT  The ID of the pipeline that will be this template's
                      parent  [required]

  --name TEXT         The name of the template  [required]
  --description TEXT  The description of the template
  --test_wdl TEXT     The location where the test WDL for this template is
                      hosted. The test WDL is the WDL which defines the thing
                      the be tested  [required]

  --eval_wdl TEXT     The location where the eval WDL for ths template is
                      hosted.  The eval WDL is the WDL which takes the outputs
                      from the test WDL and evaluates them  [required]

  --created_by TEXT   Email of the creator of the template.  Defaults to email
                      config variable

  --help              Show this message and exit.
```

### Delete
```shell
$ carrot_cli template delete --help
Usage: carrot_cli template delete [OPTIONS] ID

  Delete a template by its ID, if it has no tests associated with it

Options:
  --help  Show this message and exit.
```

### Delete report map by id
```shell
$ carrot_cli template delete_report_map_by_id --help
Usage: carrot_cli template delete_report_map_by_id [OPTIONS] ID REPORT_ID

  Delete the mapping record from the template specified by ID to the report
  specified by REPORT_ID, if the specified template has no non-failed (i.e.
  successful or currently running) runs associated with it

Options:
  --help  Show this message and exit.
```

### Delete result map by id
```shell
$ carrot_cli template delete_result_map_by_id --help
Usage: carrot_cli template delete_result_map_by_id [OPTIONS] ID RESULT_ID

  Delete the mapping record from the template specified by ID to the result
  specified by RESULT_ID, if the specified template has no non-failed (i.e.
  successful or currently running) runs associated with it

Options:
  --help  Show this message and exit.
```

### Find
```shell
$ carrot_cli template find --help
Usage: carrot_cli template find [OPTIONS]

  Retrieve templates filtered to match the specified parameters

Options:
  --template_id TEXT     The template's ID
  --pipeline_id TEXT     The ID of the pipeline that is the template's parent,
                         a version 4 UUID

  --name TEXT            The name of the template, case-sensitive
  --pipeline_name TEXT   The name of the pipeline that is the template's
                         parent, case-sensitive

  --description TEXT     The description of the template, case-sensitive
  --test_wdl TEXT        The location where the test WDL for the template is
                         hosted

  --eval_wdl TEXT        The location where the eval WDL for the template is
                         hosted

  --created_before TEXT  Upper bound for template's created_at value, in the
                         format YYYY-MM-DDThh:mm:ss.ssssss

  --created_after TEXT   Lower bound for template's created_at value, in the
                         format YYYY-MM-DDThh:mm:ss.ssssss

  --created_by TEXT      Email of the creator of the template, case sensitive
  --sort TEXT            A comma-separated list of sort keys, enclosed in
                         asc() for ascending or desc() for descending.  Ex.
                         asc(name),desc(created_at)

  --limit INTEGER        The maximum number of template records to return
                         [default: 20]

  --offset INTEGER       The offset to start at within the list of records to
                         return.  Ex. Sorting by asc(created_at) with offset=1
                         would return records sorted by when they were created
                         starting from the second record to be created
                         [default: 0]

  --help                 Show this message and exit.
```

### Find by id
```shell
$ carrot_cli template find_by_id --help
Usage: carrot_cli template find_by_id [OPTIONS] ID

  Retrieve a template by its ID

Options:
  --help  Show this message and exit.
```

### Find report map by id
```shell
$ carrot_cli template find_report_map_by_id --help
Usage: carrot_cli template find_report_map_by_id [OPTIONS] ID REPORT_ID

  Retrieve the mapping record from the template specified by ID to the
  report specified by REPORT_ID

Options:
  --help  Show this message and exit.
```

### Find report maps
```shell
$ carrot_cli template find_report_maps --help
Usage: carrot_cli template find_report_maps [OPTIONS] ID

  Retrieve the mapping record from the template specified by ID to the
  report specified by REPORT_ID

Options:
  --report_id TEXT       The id of the report
  --created_before TEXT  Upper bound for map's created_at value, in the format
                         YYYY-MM-DDThh:mm:ss.ssssss

  --created_after TEXT   Lower bound for map's created_at value, in the format
                         YYYY-MM-DDThh:mm:ss.ssssss

  --created_by TEXT      Email of the creator of the map, case sensitive
  --sort TEXT            A comma-separated list of sort keys, enclosed in
                         asc() for ascending or desc() for descending.  Ex.
                         asc(input_map),desc(report_id)

  --limit INTEGER        The maximum number of map records to return
                         [default: 20]

  --offset INTEGER       The offset to start at within the list of records to
                         return.  Ex. Sorting by asc(created_at) with offset=1
                         would return records sorted by when they were created
                         starting from the second record to be created
                         [default: 0]

  --help                 Show this message and exit.
```

### Find result map by id
```shell
$ carrot_cli template find_result_map_by_id --help
Usage: carrot_cli template find_result_map_by_id [OPTIONS] ID RESULT_ID

  Retrieve the mapping record from the template specified by ID to the
  result specified by RESULT_ID

Options:
  --help  Show this message and exit.
```

### Find result maps
```shell
$ carrot_cli template find_result_maps --help
Usage: carrot_cli template find_result_maps [OPTIONS] ID

  Retrieve the mapping record from the template specified by ID to the
  result specified by RESULT_ID

Options:
  --result_id TEXT       The id of the result
  --result_key TEXT      The key used to name the result within the output of
                         the template

  --created_before TEXT  Upper bound for map's created_at value, in the format
                         YYYY-MM-DDThh:mm:ss.ssssss

  --created_after TEXT   Lower bound for map's created_at value, in the format
                         YYYY-MM-DDThh:mm:ss.ssssss

  --created_by TEXT      Email of the creator of the map, case sensitive
  --sort TEXT            A comma-separated list of sort keys, enclosed in
                         asc() for ascending or desc() for descending.  Ex.
                         asc(result_key),desc(result_id)

  --limit INTEGER        The maximum number of map records to return
                         [default: 20]

  --offset INTEGER       The offset to start at within the list of records to
                         return.  Ex. Sorting by asc(created_at) with offset=1
                         would return records sorted by when they were created
                         starting from the second record to be created
                         [default: 0]

  --help                 Show this message and exit.
```

### Find runs
```shell
$ carrot_cli template find_runs --help
Usage: carrot_cli template find_runs [OPTIONS] ID

  Retrieve runs related to the template specified by ID, filtered by the
  specified parameters

Options:
  --name TEXT                  The name of the run
  --status TEXT                The status of the run. Status include: aborted,
                               building, created, failed, queued_in_cromwell,
                               running, starting, submitted, succeeded,
                               waiting_for_queue_space

  --test_input TEXT            A JSON file containing the inputs to the test
                               WDL for the run

  --eval_input TEXT            A JSON file containing the inputs to the eval
                               WDL for the run

  --test_cromwell_job_id TEXT  The unique ID assigned to the Cromwell job in
                               which the test WDL ran

  --eval_cromwell_job_id TEXT  The unique ID assigned to the Cromwell job in
                               which the eval WDL ran

  --created_before TEXT        Upper bound for run's created_at value, in the
                               format YYYY-MM-DDThh:mm:ss.ssssss

  --created_after TEXT         Lower bound for run's created_at value, in the
                               format YYYY-MM-DDThh:mm:ss.ssssss

  --created_by TEXT            Email of the creator of the run
  --finished_before TEXT       Upper bound for run's finished_at value, in the
                               format YYYY-MM-DDThh:mm:ss.ssssss

  --finished_after TEXT        Lower bound for run's finished_at value, in the
                               format YYYY-MM-DDThh:mm:ss.ssssss

  --sort TEXT                  A comma-separated list of sort keys, enclosed
                               in asc() for ascending or desc() for
                               descending.  Ex. asc(status),desc(created_at)

  --limit INTEGER              The maximum number of run records to return
                               [default: 20]

  --offset INTEGER             The offset to start at within the list of
                               records to return.  Ex. Sorting by
                               asc(created_at) with offset=1 would return
                               records sorted by when they were created
                               starting from the second record to be created
                               [default: 0]

  --help                       Show this message and exit.
```

### Map to report
```shell
$ carrot_cli template map_to_report --help
Usage: carrot_cli template map_to_report [OPTIONS] ID REPORT_ID

  Map the template specified by ID to the report specified by REPORT_ID

Options:
  --created_by TEXT  Email of the creator of the mapping
  --help             Show this message and exit.
```

### Map to result
```shell
$ carrot_cli template map_to_result --help
Usage: carrot_cli template map_to_result [OPTIONS] ID RESULT_ID RESULT_KEY

  Map the template specified by ID to the result specified by RESULT_ID for
  RESULT_KEY in in the output generated by that template

Options:
  --created_by TEXT  Email of the creator of the mapping
  --help             Show this message and exit.
```

### Subscribe
```shell
$ carrot_cli template subscribe --help
Usage: carrot_cli template subscribe [OPTIONS] ID

  Subscribe to receive notifications about the template specified by ID

Options:
  --email TEXT  The email address to receive notifications. If set, takes
                priority over email config variable

  --help        Show this message and exit.
```

### Unsubscribe
```shell
$ carrot_cli template unsubscribe --help
Usage: carrot_cli template unsubscribe [OPTIONS] ID

  Delete subscription to the template with the specified by ID and email

Options:
  --email TEXT  The email address to stop receiving notifications. If set,
                takes priority over email config variable

  --help        Show this message and exit.
```

### Update
```shell
$ carrot_cli template update --help
Usage: carrot_cli template update [OPTIONS] ID

  Update template with ID with the specified parameters

Options:
  --name TEXT         The name of the template
  --description TEXT  The description of the template
  --test_wdl TEXT     The location where the test WDL for the template is
                      hosted.  Updating this parameter is allowed only if the
                      specified template has no non-failed (i.e. successful or
                      currently running) runs associated with it

  --eval_wdl TEXT     The location where the eval WDL for the template is
                      hosted.  Updating this parameter is allowed only if the
                      specified template has no non-failed (i.e. successful or
                      currently running) runs associated with it

  --help              Show this message and exit.
```