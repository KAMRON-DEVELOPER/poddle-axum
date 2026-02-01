# NOTES

please when you set `tenant_id` dynamically in `loki.proccess { stage.tenant {} }`
instead inside `loki.write { }` keep the label in `stage.label_keep { values = [] }` beacuse if you drop it
it can't be sent to loki and loki says `401`
