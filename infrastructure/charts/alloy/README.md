# Loki

## [Label best practices](https://grafana.com/docs/loki/latest/get-started/labels/bp-labels/)

### Static labels are good

Use labels for things like regions, clusters, servers, applications, namespaces, and environments. They will be fixed for a given system/app and have bounded values. Use static labels to make it easier to query your logs in a logical sense (for example, show me all the logs for a given application and specific environment, or show me all the logs for all the apps on a specific host).

### Use dynamic labels sparingly

Too many label value combinations leads to too many streams. The penalties for that in Loki are a large index and small chunks in the store, which in turn can actually reduce performance.

### Label values must always be bounded

If you are dynamically setting labels, never use a label which can have unbounded or infinite values. This will always result in big problems for Loki.

Try to keep values bounded to as small a set as possible. We don’t have perfect guidance as to what Loki can handle, but think single digits, or maybe 10’s of values for a dynamic label. This is less critical for static labels. For example, if you have 1,000 hosts in your environment it’s going to be just fine to have a host label with 1,000 values.

As a general rule, you should try to keep any single tenant in Loki to less than 100,000 active streams, and less than a million streams in a 24-hour period. These values are for HUGE tenants, sending more than 10 TB a day. If your tenant is 10x smaller, you should have at least 10x fewer labels.

## [Cardinality](https://grafana.com/docs/loki/latest/get-started/labels/cardinality/>)

The cardinality of a data attribute is the number of distinct values that the attribute can have. For example, a boolean column in a database, which can only have a value of either true or false has a cardinality of 2.

In Loki 1.6.0 and newer the logcli series command added the --analyze-labels flag specifically for debugging high cardinality labels:

```bash
Total Streams:  25017
Unique Labels:  8

Label Name  Unique Values  Found In Streams
requestId   24653          24979
logStream   1194           25016
logGroup    140            25016
accountId   13             25016
logger      1              25017
source      1              25016
transport   1              25017
format      1              25017
```

To view the cardinality of your current labels, you can use logcli.

```bash
logcli series '{}' --since=1h --analyze-labels
```

> [!NOTE]
>Structured metadata is a feature in Loki and Cloud Logs that allows customers to store
>
>metadata that is too high cardinality for log lines, without needing to embed that information in
>
> log lines themselves.
>
> It is a great home for metadata which is not easily embeddable in a log line, but is too high
>
> cardinality to be used effectively as a label.

## [Structured metadata](https://grafana.com/docs/loki/latest/get-started/labels/structured-metadata/)

Selecting proper, low cardinality labels is critical to operating and querying Loki effectively. Some metadata, especially infrastructure related metadata, can be difficult to embed in log lines, and is too high cardinality to effectively store as indexed labels (and therefore reducing performance of the index).

Structured metadata is a way to attach metadata to logs without indexing them or including them in the log line content itself. Examples of useful metadata are kubernetes pod names, process ID’s, or any other label that is often used in queries but has high cardinality and is expensive to extract at query time.

Structured metadata can also be used to query commonly needed metadata from log lines without needing to apply a parser at query time. Large json blobs or a poorly written query using complex regex patterns, for example, come with a high performance cost. Examples of useful metadata include container_IDs or user IDs.

### Enable or disable structured metadata

You enable structured metadata in the Loki config.yaml file.

```bash
limits_config:
    allow_structured_metadata: true
    volume_enabled: true
    retention_period: 672h # 28 days retention
```

or

```bash
loki:
    # -- Limits config
  limits_config:
    reject_old_samples: true
    reject_old_samples_max_age: 168h
    max_cache_freshness_per_query: 10m
    split_queries_by_interval: 15m
    query_timeout: 300s
    allow_structured_metadata: true
    volume_enabled: true
    # retention_period: 30d
```
