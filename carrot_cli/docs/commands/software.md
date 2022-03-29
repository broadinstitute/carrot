---
layout: default
title: software
description: "Commands for searching, creating, and updating software definitions"
nav_order: 7
has_children: true
parent: Commands
has_toc: false
---

# Software
{: .no_toc}

## Table of contents
{: .no_toc .text-delta}

* TOC
{:toc}

---

## Description

In CARROT, software represents a piece of software or software pipeline to test. It is not strictly necessary to define a software unit in order to define and run tests, but it is highly encouraged because of the benefits it affords.

Creating a software record is basically registering a git repo with CARROT so it can be referenced in test/run inputs, allowing dynamic generation of docker images from arbitrary git commits for running your tests.

For more information on how to use software records with your tests, see the [Setting Up Tests section](https://github.com/broadinstitute/carrot/blob/master/UserGuide.md#setting-up-tests-in-carrot) in the [CARROT User Guide](https://github.com/broadinstitute/carrot/blob/master/UserGuide.md).

## Commands

### Create
```shell
$ carrot_cli software create --help
Usage: carrot_cli software create [OPTIONS]

  Create software definition with the specified parameters

Options:
  --name TEXT            The name of the software  [required]
  --description TEXT     The description of the software
  --repository_url TEXT  The url to use for cloning the repository.
                         [required]

  --created_by TEXT      Email of the creator of the software.  Defaults to
                         email config variable

  --help                 Show this message and exit.
```

### Find
```shell
$ carrot_cli software find --help
Usage: carrot_cli software find [OPTIONS]

  Retrieve software definitions filtered to match the specified parameters

Options:
  --software_id TEXT     The software's ID
  --name TEXT            The name of the software, case-sensitive
  --description TEXT     The description of the software, case-sensitive
  --repository_url TEXT  The url of the repository where the software code is
                         hosted

  --created_before TEXT  Upper bound for software's created_at value, in the
                         format YYYY-MM-DDThh:mm:ss.ssssss

  --created_after TEXT   Lower bound for software's created_at value, in the
                         format YYYY-MM-DDThh:mm:ss.ssssss

  --created_by TEXT      Email of the creator of the software, case sensitive
  --sort TEXT            A comma-separated list of sort keys, enclosed in
                         asc() for ascending or desc() for descending.  Ex.
                         asc(name),desc(created_at)

  --limit INTEGER        The maximum number of software records to return
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
$ carrot_cli software find_by_id --help
Usage: carrot_cli software find_by_id [OPTIONS] ID

  Retrieve a software definition by its ID

Options:
  --help  Show this message and exit.
```

### Update
```shell
$ carrot_cli software update --help
Usage: carrot_cli software update [OPTIONS] ID

  Update software definition with ID with the specified parameters

Options:
  --name TEXT         The name of the software
  --description TEXT  The description of the software
  --help              Show this message and exit.

```