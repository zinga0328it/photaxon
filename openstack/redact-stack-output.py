#!/usr/bin/env python3
import os
import sys

secret_names = (
    "ADMIN_PASSWORD",
    "DATABASE_PASSWORD",
    "RABBIT_PASSWORD",
    "SERVICE_PASSWORD",
)
secrets = [os.environ.get(name, "") for name in secret_names]

for line in sys.stdin:
    for secret in secrets:
        if secret:
            line = line.replace(secret, "<REDACTED>")
    sys.stdout.write(line)
