#!/usr/bin/bash

#this is instead of 'spacetime login' which wants to go on the internet! src: https://spacetimedb.com/docs/getting-started

servernick="autoslocalscripts"
spacetime server remove "$servernick"
spacetime server add --no-fingerprint -d --url https://127.1.2.3:6543 "$servernick" \
 --client-cert ../spacetimedb-cert-gen/client1.crt --client-key ../spacetimedb-cert-gen/client1.key --trust-server-cert ../spacetimedb-cert-gen/ca.crt
spacetime server list

spacetime logout
#spacetime login --server-issued-login https://127.1.2.3:6543 --cert ../spacetimedb-cert-gen/ca.crt
#^ bugged as: https://github.com/clockworklabs/SpacetimeDB/issues/2512
spacetime login --server-issued-login "$servernick" \
 --client-cert ../spacetimedb-cert-gen/client1.crt --client-key ../spacetimedb-cert-gen/client1.key --trust-server-cert ../spacetimedb-cert-gen/ca.crt
