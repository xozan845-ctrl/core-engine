$queries = @(
    @{name="Servicios UP"; q="count(up{job=`"core-engine-servicios`"} == 1)"},
    @{name="Servicios DOWN"; q="count(up{job=`"core-engine-servicios`"} == 0)"},
    @{name="Latencia p99"; q="histogram_quantile(0.99, sum by (servicio, le) (rate(http_peticion_latencia_ms_bucket[5m])))"},
    @{name="Error Rate 5xx%"; q="sum(rate(http_peticiones_total{status=~`"5..`"}[5m])) / sum(rate(http_peticiones_total[5m])) * 100"},
    @{name="Event Loop Lag p99"; q="nodejs_eventloop_lag_p99_seconds{job=`"core-engine-servicios`"} * 1000"},
    @{name="Heap Used MB"; q="nodejs_heap_size_used_bytes{job=`"core-engine-servicios`"} / 1048576"},
    @{name="GC Major Avg ms"; q="rate(nodejs_gc_duration_seconds_sum{kind=`"major`"}[5m]) / rate(nodejs_gc_duration_seconds_count{kind=`"major`"}[5m]) * 1000"},
    @{name="PG Conexiones"; q="pg_stat_database_numbackends{datname=`"core-engine`"}"},
    @{name="PG Cache Hit%"; q="pg_stat_database_blks_hit / (pg_stat_database_blks_hit + pg_stat_database_blks_read) * 100"},
    @{name="PG Dead Tuples"; q="sum(pg_stat_user_tables_n_dead_tup)"},
    @{name="RMQ Backlog"; q="max(rabbitmq_queue_messages)"},
    @{name="RMQ Consumers"; q="sum(rabbitmq_queue_consumers)"},
    @{name="Disk Usage%"; q="max by (instance) ((node_filesystem_size_bytes - node_filesystem_avail_bytes) / node_filesystem_size_bytes) * 100"}
)

Write-Host "=== VERIFICACIÓN FINAL MÉTRICAS CLAVE ==="
foreach ($item in $queries) {
    $r = curl.exe -G "http://localhost:9090/api/v1/query" --data-urlencode "query=$($item.q)" | ConvertFrom-Json
    if ($r.data.result.Count -gt 0) {
        $val = $r.data.result[0].value[1]
        if ($val -eq "NaN") { Write-Host "  [$($item.name)] NaN" }
        else { Write-Host "  [$($item.name)] $([math]::Round([double]$val,2))" }
    } else {
        Write-Host "  [$($item.name)] NO DATA"
    }
}