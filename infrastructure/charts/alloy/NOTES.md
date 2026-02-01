# NOTES

please when you set `tenant_id` dynamically in `loki.proccess { stage.tenant {} }`
instead inside `loki.write { }` which is static keep the label in `stage.label_keep { values = [] }`
beacuse if you don't tenant id never set and loki says `401`.
Also you must place it end of the `loki.process` component.
