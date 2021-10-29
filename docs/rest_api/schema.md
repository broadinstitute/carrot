---
layout: default
title: schema
description: "Schema reference"
nav_order: 14
parent: REST API
---

# Schema
{: .no_toc}

## Table of contents
{: .no_toc .text-delta}

* TOC
  {:toc}

---

## Description

This page contains documentation for the types of data that can be returned by the CARROT REST API.

## Schema Objects

### Pipeline

|Name|Description|Schema|
|---|---|---|
|**pipeline_id**|Unique ID for the pipeline <br>**Example**: `"3c6fbe2e-d58b-410d-ab13-c0f67bd94ce5"`|string (uuid)|
|**name**|The name of the pipeline <br>**Example**: `"My cool pipeline"`|string|
|**description** <br>*optional*|A description for the pipeline <br>**Example**: `"I made this pipeline myself for testing a workflow"`|string|
|**created_at**|Datetime of the creation of the pipeline <br>**Example**: `"2020-11-20T07:55:24.234512"`|string (date-time)|
|**created_by** <br>*optional*|Email address of the creator of the pipeline <br>**Example**: `"creator@example.com"`|string (email)|

### ErrorBody

|Name|Description|Schema|
|---|---|---|
|**title**|A short message explaining the error <br>**Example**:`""ID formatted incorrectly""`|string|
|**status**|The status code of the response <br>**Example**: `400`|int|
|**detail**|A more detailed error message <br>**Example**: `"ID must be formatted as a Uuid"`|string|
