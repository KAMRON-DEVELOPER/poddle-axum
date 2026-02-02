# NOTES

please when you set `tenant_id` dynamically in `loki.proccess { stage.tenant {} }`
instead inside `loki.write { }` which is static keep the label in `stage.label_keep { values = [] }`
beacuse if you don't tenant id never set and loki says `401`.
Also you must place it end of the `loki.process` component.

```bash
export LOKI_ADDR=https://loki.poddle.uz
```

```bash
export LOKI_ORG_ID=680377c6-0a6c-4cd0-a663-fd97d1a57332
```

```bash
curl -sS -G "https://loki.poddle.uz/loki/api/v1/query_range" \
  --data-urlencode 'query={deployment_id="5b64d859-aa08-4840-ab47-6939e685d6d3",project_id="abf56546-37b5-4264-a104-15c05c237ff6"}' \
  --data-urlencode "start=$(date -d '5 minutes ago' +%s)000000000" \
  --data-urlencode "end=$(date +%s)000000000" \
  -H "X-Scope-OrgID: 680377c6-0a6c-4cd0-a663-fd97d1a57332" | jq 'del(.data.stats)'
```
