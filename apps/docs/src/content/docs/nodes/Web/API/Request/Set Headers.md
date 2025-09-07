---
title: Set Headers
description: A node to set headers on an HTTP request.
---

## Purpose of the Node
The Set Headers node allows you to add or update headers in an HTTP request.

## Pins
The node has three pins:

| Pin Name | Pin Description | Pin Type | Value Type |
|:----------:|:-------------:|:------:|:------:|
| request | The HTTP request | Struct | HttpRequest |
| headers | The headers to be set | String | HashMap |
| request_out | The HTTP request with updated headers | Struct | HttpRequest |