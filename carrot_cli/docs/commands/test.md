---
layout: default
title: test
description: "Commands for searching, creating, and updating tests"
nav_order: 5
parent: Commands
---

# Test
{: .no_toc}

## Table of contents
{: .no_toc .text-delta}

* TOC
{:toc}

---

## Description

A CARROT test is an instance of a template with inputs filled in. Specifically, any inputs that define input data and / or ground truth data from which to create outputs and points of comparison for evaluation. Typically all inputs will be defined in the test with the exception of the docker container that contains a specific software/pipeline version. The typical use case is to vary the docker container for a fixed test so that the analysis can be tracked as new software versions are released.

## Commands

### Create
```shell
$ carrot_cli test create --help
Usage: carrot_cli test create [OPTIONS]

  Create test with the specified parameters

Options:
  --name TEXT                 The name of the test  [required]
  --template_id TEXT          The ID of the template that will be the test's
                              parent  [required]

  --description TEXT          The description of the test
  --test_input_defaults TEXT  A JSON file containing the default inputs to the
                              test WDL for the test

  --eval_input_defaults TEXT  A JSON file containing the default inputs to the
                              eval WDL for the test

  --created_by TEXT           Email of the creator of the test.  Defaults to
                              email config variable

  --help                      Show this message and exit.
```

### Delete
```shell
$ carrot_cli test delete --help
Usage: carrot_cli test delete [OPTIONS] ID

  Delete a test by its ID, if the test has no runs associated with it

Options:
  --help  Show this message and exit.
```

### Find
```shell
$ carrot_cli test find --help
Usage: carrot_cli test find [OPTIONS]

  Retrieve tests filtered to match the specified parameters

Options:
  --test_id TEXT              The test's ID
  --template_id TEXT          The ID of the template that is the test's
                              parent

  --name TEXT                 The name of the test, case-sensitive
  --template_name TEXT        The name of the template that is the test's
                              parent, case-sensitive

  --description TEXT          The description of the test, case-sensitive
  --test_input_defaults TEXT  A JSON file containing the default inputs to the
                              test WDL for the test

  --eval_input_defaults TEXT  A JSON file containing the default inputs to the
                              eval WDL for the test

  --created_before TEXT       Upper bound for test's created_at value, in the
                              format YYYY-MM-DDThh:mm:ss.ssssss

  --created_after TEXT        Lower bound for test's created_at value, in the
                              format YYYY-MM-DDThh:mm:ss.ssssss

  --created_by TEXT           Email of the creator of the test, case sensitive
  --sort TEXT                 A comma-separated list of sort keys, enclosed in
                              asc() for ascending or desc() for descending.
                              Ex. asc(name),desc(created_at)

  --limit INTEGER             The maximum number of test records to return
                              [default: 20]

  --offset INTEGER            The offset to start at within the list of
                              records to return.  Ex. Sorting by
                              asc(created_at) with offset=1 would return
                              records sorted by when they were created
                              starting from the second record to be created
                              [default: 0]

  --help                      Show this message and exit.
```

### Find by id
```shell
$ carrot_cli test find_by_id --help
Usage: carrot_cli test find_by_id [OPTIONS] ID

  Retrieve a test by its ID

Options:
  --help  Show this message and exit.
```

### Find runs
```shell
$ carrot_cli test find_runs --help
Usage: carrot_cli test find_runs [OPTIONS] ID

  Retrieve runs of the test specified by ID, filtered by the specified
  parameters

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

### Run
```shell
$ carrot_cli test run --help
Usage: carrot_cli test run [OPTIONS] ID

  Start a run for the test specified by ID with the specified params

Options:
  --name TEXT        The name of the run.  Will be autogenerated if not
                     specified

  --test_input TEXT  A JSON file containing the inputs to the test WDL for the
                     run

  --eval_input TEXT  A JSON file containing the inputs to the eval WDL for the
                     run

  --created_by TEXT  Email of the creator of the run.  Defaults to email
                     config variable

  --help             Show this message and exit.
```

### Subscribe
```shell
$ carrot_cli test subscribe --help
Usage: carrot_cli test subscribe [OPTIONS] ID

  Subscribe to receive notifications about the test specified by ID

Options:
  --email TEXT  The email address to receive notifications. If set, takes
                priority over email config variable

  --help        Show this message and exit.
```

### Unsubscribe
```shell
$ carrot_cli test unsubscribe --help
Usage: carrot_cli test unsubscribe [OPTIONS] ID

  Delete subscription to the test with the specified by ID and email

Options:
  --email TEXT  The email address to stop receiving notifications. If set,
                takes priority over email config variable

  --help        Show this message and exit.
```

### Update
```shell
$ carrot_cli test update --help
Usage: carrot_cli test update [OPTIONS] ID

  Update test with ID with the specified parameters

Options:
  --name TEXT                 The name of the test
  --description TEXT          The description of the test
  --test_input_defaults TEXT  A JSON file containing the default inputs to the
                              test WDL for the test. Updating this parameter
                              is allowed only if the specified test has no
                              non-failed (i.e. successful or currently
                              running) runs associated with it

  --eval_input_defaults TEXT  A JSON file containing the default inputs to the
                              eval WDL for the test. Updating this parameter
                              is allowed only if the specified test has no
                              non-failed (i.e. successful or currently
                              running) runs associated with it

  --help                      Show this message and exit.
```
