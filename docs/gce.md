# Running on GCE

## Coordinator

### Coordinator Size

Machine type: f1-micro, preemptible

### Coordinator Firewall rules

Need `tcp:5555,5556` open to coordinator.

### Coordinator Instance Metadata

* Set `minuteman-version` to the desired minuteman release. (e.g., `0.1.1`)

### Coordinator Startup Script

```
#!/bin/sh

apt update
apt install wget
cd /tmp
MINUTEMAN_VERSION="$(curl http://metadata.google.internal/computeMetadata/v1/instance/attributes/minuteman-version -H "Metadata-Flavor: Google")"
wget https://github.com/bsundsrud/minuteman/releases/download/${MINUTEMAN_VERSION}/minuteman-x86_64-linux-${MINUTEMAN_VERSION}.tar.gz
tar xzf minuteman-x86_64-linux-${MINUTEMAN_VERSION}.tar.gz
cd minuteman
./install-coordinator.sh
systemctl start minuteman-coordinator
```

## Worker

### Worker Size

Machine type: c2-standard-4

### Worker Instance Metadata

* Set `minuteman-version` to the desired minuteman release. (e.g., `0.1.1`)
* Set `coordinator-url` to the coordinator's websocket URL (e.g., `ws://10.0.0.2:5556`)

### Worker Startup Script

```
#!/bin/sh

apt update
apt install wget
cd /tmp
MINUTEMAN_VERSION="$(curl http://metadata.google.internal/computeMetadata/v1/instance/attributes/minuteman-version -H "Metadata-Flavor: Google")"
COORDINATOR_URL="$(curl http://metadata.google.internal/computeMetadata/v1/instance/attributes/coordinator-url -H "Metadata-Flavor: Google")"
wget https://github.com/bsundsrud/minuteman/releases/download/${MINUTEMAN_VERSION}/minuteman-x86_64-linux-${MINUTEMAN_VERSION}.tar.gz
tar xzf minuteman-x86_64-linux-${MINUTEMAN_VERSION}.tar.gz
cd minuteman
./install-worker.sh "$COORDINATOR_URL"
systemctl start minuteman-worker
```
