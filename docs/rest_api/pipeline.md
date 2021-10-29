---
layout: default
title: pipeline
description: "Creating, updating, and deleting pipelines"
nav_order: 2
parent: REST API
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

## Mappings

### Find by ID
Retrieve a pipeline from the CARROT database by its ID.

```http request
POST /api/v1/pipelines/{id}
```
**Consumes:** `application/json`

**Parameters:**

|Type|Name|Description|Schema|
|---|---|---|---|
|**Path**|**id** <br>*required*|Unique ID for the pipeline|string

**Produces:** `application/json`

**Responses:**

|HTTP Code|Description|Schema|
|---|---|---|
|**200**|Success|[Pipeline](/carrot/rest_api/schema#pipeline)|
|**400**|Bad request|[ErrorBody](/carrot/rest_api/schema#errorbody)|
|**404**|Not found|[ErrorBody](/carrot/rest_api/schema#errorbody)|
|**500**|Server Error|[ErrorBody](/carrot/rest_api/schema#errorbody)|
