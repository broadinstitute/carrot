---
layout: default
title: config
description: "Commands for setting and displaying config variables"
nav_order: 1
parent: Commands
---

# Config
{: .no_toc}

## Table of contents
{: .no_toc .text-delta}

* TOC
{:toc}

---

## Description

There are two config variables that must be set before using carrot_cli.  They are:
* carrot_server_address - the address of the CARROT server you'd like to connect to
* email - your email address, for provenance and notification purposes

The values specified for these variables are stored locally in `.carrot_cli/config.json` within your home directory.

## Commands

### Get
```shell
$ carrot_cli config get --help
Usage: carrot_cli config get [OPTIONS]

  Prints the current config

Options:
  --help  Show this message and exit.
```

### Set
```shell
$ carrot_cli config set --help
Usage: carrot_cli config set [OPTIONS] VARIABLE VALUE

  Set the value of a specified config variable

Options:
  --help  Show this message and exit.

```