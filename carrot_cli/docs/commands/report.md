---
layout: default
title: report
description: "Commands for searching, creating, and updating reports"
nav_order: 8
parent: Commands
---

# Report
{: .no_toc}

## Table of contents
{: .no_toc .text-delta}

* TOC
{:toc}

---

## Description

Jupyter Notebook reports can be generated from successful runs. These reports serve as visualizations of the CARROT run results and should be created to display relevant data in a readable, straight-forward manner. They can be generated either automatically (when a run finishes successfully) or manually.

Before a report can be generated, it must be defined by creating a CARROT report.

## Commands

### Create
```shell
$ carrot_cli report create --help
Usage: carrot_cli report create [OPTIONS]

  Create report with the specified parameters

Options:
  --name TEXT         The name of the report  [required]
  --description TEXT  The description of the report
  --notebook TEXT     The ipynb file containing the notebook which will serve
                      as a template for this report.  [required]

  --config TEXT       A json file containing values for runtime attributes for
                      the Cromwell job that will generate the report.  The
                      allowed attributes are: cpu, memory, disks, docker,
                      maxRetries, continueOnReturnCode, failOnStderr,
                      preemptible, and bootDiskSizeGb.

  --created_by TEXT   Email of the creator of the report.  Defaults to email
                      config variable

  --help              Show this message and exit.
```

### Delete
```shell
$ carrot_cli report delete --help
Usage: carrot_cli report delete [OPTIONS] ID

  Delete a report by its ID, if the report has no templates, sections, or
  runs associated with it.

Options:
  --help  Show this message and exit.
```

### Find
```shell
$ carrot_cli report find --help
Usage: carrot_cli report find [OPTIONS]

  Retrieve reports filtered to match the specified parameters

Options:
  --report_id TEXT       The report's ID
  --name TEXT            The name of the report, case-sensitive
  --description TEXT     The description of the report, case-sensitive
  --notebook TEXT        The ipynb file containing the notebook for the
                         report.

  --config TEXT          A json file containing values for runtime attributes
                         for the Cromwell job that runs the report.

  --created_before TEXT  Upper bound for report's created_at value, in the
                         format YYYY-MM-DDThh:mm:ss.ssssss

  --created_after TEXT   Lower bound for report's created_at value, in the
                         format YYYY-MM-DDThh:mm:ss.ssssss

  --created_by TEXT      Email of the creator of the report, case sensitive
  --sort TEXT            A comma-separated list of sort keys, enclosed in
                         asc() for ascending or desc() for descending.  Ex.
                         asc(name),desc(created_at)

  --limit INTEGER        The maximum number of report records to return
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
$ carrot_cli report find_by_id --help
Usage: carrot_cli report find_by_id [OPTIONS] ID

  Retrieve a report by its ID

Options:
  --help  Show this message and exit.
```

### Update
```shell
$ carrot_cli report update --help
Usage: carrot_cli report update [OPTIONS] ID

  Update report with ID with the specified parameters

Options:
  --name TEXT         The name of the report
  --description TEXT  The description of the report
  --notebook TEXT     The ipynb file containing the notebook which will serve
                      as a template for this report.

  --config TEXT       A json file containing values for runtime attributes for
                      the Cromwell job that will generate the report.  The
                      allowed attributes are: cpu, memory, disks, docker,
                      maxRetries, continueOnReturnCode, failOnStderr,
                      preemptible, and bootDiskSizeGb.

  --help              Show this message and exit.
```