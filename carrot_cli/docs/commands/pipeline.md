---
layout: default
title: pipeline
description: "Creating, updating, and deleting pipelines"
nav_order: 2
parent: Commands
---

# Pipeline
{: .no_toc}

## Table of contents
{: .no_toc .text-delta}

* TOC
{:toc}

---

## Description

A CARROT pipeline defines a unit on which analyses can be performed. This unit can be a piece of software or several pieces of software connected together to perform a task to be analyzed.

## Commands

### Create
```shell
$ carrot_cli pipeline create --help
Usage: carrot_cli pipeline create [OPTIONS]

  Create pipeline with the specified parameters

Options:
  --name TEXT         The name of the pipeline  [required]
  --description TEXT  The description of the pipeline
  --created_by TEXT   Email of the creator of the pipeline.  Defaults to email
                      config variable

  --help              Show this message and exit.
```

### Delete
```shell
$ carrot_cli pipeline delete --help
Usage: carrot_cli pipeline delete [OPTIONS] ID

  Delete a pipeline by its ID, if the pipeline has no templates associated
  with it.

Options:
  --help  Show this message and exit.
```

### Find
```shell
$ carrot_cli pipeline find --help
Usage: carrot_cli pipeline find [OPTIONS]

  Retrieve pipelines filtered to match the specified parameters

Options:
  --pipeline_id TEXT     The pipeline's ID
  --name TEXT            The name of the pipeline, case-sensitive
  --description TEXT     The description of the pipeline, case-sensitive
  --created_before TEXT  Upper bound for pipeline's created_at value, in the
                         format YYYY-MM-DDThh:mm:ss.ssssss

  --created_after TEXT   Lower bound for pipeline's created_at value, in the
                         format YYYY-MM-DDThh:mm:ss.ssssss

  --created_by TEXT      Email of the creator of the pipeline, case sensitive
  --sort TEXT            A comma-separated list of sort keys, enclosed in
                         asc() for ascending or desc() for descending.  Ex.
                         asc(name),desc(created_at)

  --limit INTEGER        The maximum number of pipeline records to return
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
$ carrot_cli pipeline find_by_id --help
Usage: carrot_cli pipeline find_by_id [OPTIONS] ID

  Retrieve a pipeline by its ID

Options:
  --help  Show this message and exit.
```

### Find runs
```shell
$ carrot_cli pipeline find_runs --help
Usage: carrot_cli pipeline find_runs [OPTIONS] ID

  Retrieve runs related to the pipeline specified by ID, filtered by the
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

### Subscribe
```shell
$ carrot_cli pipeline subscribe --help
Usage: carrot_cli pipeline subscribe [OPTIONS] ID

  Subscribe to receive notifications about the pipeline specified by ID

Options:
  --email TEXT  The email address to receive notifications. If set, takes
                priority over email config variable

  --help        Show this message and exit.

```

### Unsubscribe
```shell
$ carrot_cli pipeline unsubscribe --help
Usage: carrot_cli pipeline unsubscribe [OPTIONS] ID

  Delete subscription to the pipeline with the specified by ID and email

Options:
  --email TEXT  The email address to stop receiving notifications. If set,
                takes priority over email config variable

  --help        Show this message and exit.

```

### Update
```shell
$ carrot_cli pipeline update --help
Usage: carrot_cli pipeline update [OPTIONS] ID

  Update pipeline with ID with the specified parameters

Options:
  --name TEXT         The name of the pipeline
  --description TEXT  The description of the pipeline
  --help              Show this message and exit.
```