#!/bin/bash
set -e

# Wait for the SQLite database to be ready
sleep 2

# Skip if already initialized
if pdnsutil list-zone shippin.tech.test >/dev/null 2>&1; then
    echo "Zone shippin.tech.test already exists; skipping init."
    exit 0
fi

# Create the zone with default SOA + NS
pdnsutil create-zone shippin.tech.test ns1.shippin.tech.test
pdnsutil set-kind shippin.tech.test native

# Replace the default SOA with our own
pdnsutil replace-rrset shippin.tech.test '' SOA 'ns1.shippin.tech.test. admin.shippin.tech.test. 2024010101 3600 900 604800 86400'

# Add basic records
pdnsutil add-record shippin.tech.test 'ns1' A '127.0.0.1'
pdnsutil add-record shippin.tech.test '' A '127.0.0.1'
pdnsutil add-record shippin.tech.test '' MX '10 mail.shippin.tech.test.'
pdnsutil add-record shippin.tech.test 'mail' A '127.0.0.1'

# Add a sample TLSA record
# Usage=3 (DANE-EE), Selector=1 (SubjectPublicKeyInfo), Matching=1 (SHA-256)
pdnsutil add-record shippin.tech.test '_25._tcp.mail' TLSA '3 1 1 0000000000000000000000000000000000000000000000000000000000000000'

# Import static TSIG key for RFC2136 dynamic updates
# Key: shippin.tech-update-key / HMAC-SHA256
# Base64 secret: c3RhbHdhcnQtdGVzdC10c2lnLXNlY3JldC1rZXkxMjM0NTY3ODkw
pdnsutil import-tsig-key shippin.tech-update-key hmac-sha256 'c3RhbHdhcnQtdGVzdC10c2lnLXNlY3JldC1rZXkxMjM0NTY3ODkw'
pdnsutil activate-tsig-key shippin.tech.test shippin.tech-update-key primary
pdnsutil set-meta shippin.tech.test TSIG-ALLOW-DNSUPDATE shippin.tech-update-key
pdnsutil set-meta shippin.tech.test ALLOW-DNSUPDATE-FROM '0.0.0.0/0'


echo "PowerDNS zone setup complete."
echo "TSIG key name:      shippin.tech-update-key"
echo "TSIG algorithm:     hmac-sha256"
echo "TSIG secret (b64):  c3RhbHdhcnQtdGVzdC10c2lnLXNlY3JldC1rZXkxMjM0NTY3ODkw"
