#!/bin/sh
exec /dyndns/dyndns
if [ "${DEBUG}" = "true" ]; then
    while true
    do
        echo "exec /dyndns/dyndns fail"
        sleep 1000
    done
fi
