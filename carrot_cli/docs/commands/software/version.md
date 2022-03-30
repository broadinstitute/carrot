---
layout: default
title: version
description: "Commands for querying software version records"
nav_order: 1
parent: software
grand_parent: Commands
---

# Version
{: .no_toc}

## Table of contents
{: .no_toc .text-delta}

* TOC
{:toc}

---

## Description

A version refers to a specific version of a software, identified by either a git commit hash or tag.

## Commands

### Find
```shell
$ carrot_cli software version find --help
Usage: carrot_cli software version find [OPTIONS]

  Retrieve software version records filtered to match the specified
  parameters

Options:
  --software_version_id TEXT  The ID of the software version record, a version
                              4 UUID

  --software_id TEXT          The ID of the software to find version records
                              of

  --commit TEXT               The commit hash for the version
  --software_name TEXT        The name of the software to find version records
                              of, case-sensitive

  --created_before TEXT       Upper bound for software version's created_at
                              value, in the format YYYY-MM-DDThh:mm:ss.ssssss

  --created_after TEXT        Lower bound for software version's created_at
                              value, in the format YYYY-MM-DDThh:mm:ss.ssssss

  --sort TEXT                 A comma-separated list of sort keys, enclosed in
                              asc() for ascending or desc() for descending.
                              Ex. asc(software_name),desc(created_at)

  --limit INTEGER             The maximum number of software version records
                              to return  [default: 20]

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
$ carrot_cli software version find_by_id --help
Usage: carrot_cli software version find_by_id [OPTIONS] ID

  Retrieve a software version record by its ID

Options:
  --help  Show this message and exit.
```