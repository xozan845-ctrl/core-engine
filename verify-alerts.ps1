$checks = @(
    @{name="Servicios UP"; query="count(up{job=`"bodegahub-servicios`"} == 1)"},
    @{name="Servicios caidos"; query="count(up{job=`"bodegahub-servicios`"} == 0)"},
    @{name="Latencia p99"; query="histogram_quantile(0.99, sum by (servicio, le) (rate(http_peticion_latencia_ms_bucket[5m])))"},
    @{name="Error Rate 5xx%"; query="sum(rate(http_peticiones_total{status=~`"5..`"}[5m])) / sum(rate(http_peticiones_total[5m])) * 100"},
    @{name="Event Loop Lag p99"; query="nodejs_eventloop_lag_p99_seconds{job=~`"bodegahub-servicios`"} * 1000"},
    @{name="GC Major Avg ms"; query="rate(nodejs_gc_duration_seconds_sum{kind=`"major`"}[5m]) / rate(nodejs_gc_duration_seconds_count{kind=`"major`"}[5m]) * 1000"},
    @{name="Heap Used MB"; query="nodejs_heap_size_used_bytes{job=~`"bodegahub-servicios`"} / 1048576"},
    @{name="Conexiones PG"; query="pg_stat_database_numbackends{datname=`"bodegahub`"}"},
    @{name="Cache Hit PG%"; query="pg_stat_database_blks_hit / (pg_stat_database_blks_hit + pg_stat_database_blks_read) * 100"},
    @{name="Dead Tuples PG"; query="sum(pg_stat_user_tables_n_dead_tup)"},
    @{name="RabbitMQ Backlog"; query="max(rabbitmq_queue_messages)"},
    @{name="RabbitMQ Consumers"; query="sum(rabbitmq_queue_consumers)"},
    @{name="Disk Usage%"; query="max by (instance) ((node_filesystem_size_bytes - node_filesystem_avail_bytes) / node_filesystem_size_bytes) * 100"}
)

Write-Host "=== DASHBOARD 2: Alerts & Health Summary ==="
foreach ($c in $checks) {
    $r = curl.exe -s "http://localhost:9090/api/v1/query?query=$($c.query)" | ConvertFrom-Json
    if ($r.data.result.Count -gt 0) {
        $val = $r.data.result[0].value[1]
        if ($val -eq "NaN") { Write-Host "  [$($c.name)] NaN" }
        else { Write-Host "  [$($c.name)] $([math]::Round([double]$val,2))" }
    } else {
        Write-Host "  [$($c.name)] NO DATA"
    }
}