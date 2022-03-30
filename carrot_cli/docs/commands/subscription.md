---
layout: default
title: subscription
description: "Commands for searching subscriptions"
nav_order: 9
parent: Commands
---

# Subscription
{: .no_toc}

## Table of contents
{: .no_toc .text-delta}

* TOC
{:toc}

---

## Description
CARROT provides functionality for subscribing to certain entities (pipelines, templates, tests) to receive email notifications for events related to runs and reports belonging to those entities.  

Subscribing and unsubscribing is done using commands in the command groups for those entities.  The `subscription` command group provides commands for searching through subscriptions.

This allows for answering questions like, "What am I currently subscribed to?" and "Who is subscribed to this test I made?"

## Commands

### Find
```shell
$ carrot_cli subscription find --help
Usage: carrot_cli subscription find [OPTIONS]

  Retrieve subscriptions filtered to match the specified parameters

Options:
  --subscription_id TEXT  The subscription's ID
  --entity_type TEXT      The type of the entity subscribed to (pipeline,
                          template, or test)

  --entity_id TEXT        The entity's ID
  --created_before TEXT   Upper bound for subscription's created_at value, in
                          the format YYYY-MM-DDThh:mm:ss.ssssss

  --created_after TEXT    Lower bound for subscription's created_at value, in
                          the format YYYY-MM-DDThh:mm:ss.ssssss

  --email TEXT            Email of the subscriber, case sensitive
  --sort TEXT             A comma-separated list of sort keys, enclosed in
                          asc() for ascending or desc() for descending.  Ex.
                          asc(entity_type),desc(entity_id)

  --limit INTEGER         The maximum number of subscription records to return
                          [default: 20]

  --offset INTEGER        The offset to start at within the list of records to
                          return.  Ex. Sorting by asc(created_at) with
                          offset=1 would return records sorted by when they
                          were created starting from the second record to be
                          created  [default: 0]

  --help                  Show this message and exit.

```

### Find by id
```shell
$ carrot_cli subscription find_by_id --help
Usage: carrot_cli subscription find_by_id [OPTIONS] ID

  Retrieve a subscription by its ID

Options:
  --help  Show this message and exit.

```