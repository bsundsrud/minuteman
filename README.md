# Minuteman

**Please don't be evil**

[Minuteman](https://en.wikipedia.org/wiki/LGM-30_Minuteman) is an elastic load testing tool.

## Installing

Releases can be found on the [Releases](https://github.com/bsundsrud/minuteman/releases) page.  Currently, there are only prebuilt binaries for Linux x64.

## Overview

Minuteman is divided into one coordinator and *n* workers.  The coordinator and workers communicate over
WebSockets, whereby the coordinator sends commands (such as Start, Stop, Reset Stats) and the workers
send statistics to the coordinator on a heartbeat.

The coordinator starts two web servers: one listening on 5555 for the control UI, and one on 5556 for
WebSocket traffic between the workers and the coordinator.

## Usage

### Coordinator

Start the coordinator by running `minuteman` without arguments.  The UI/API can be accessed on port 5555.

### Worker

Start a worker by running `minuteman ws://<coordinator-host-or-ip>:5556`. The worker will run until
interrupted or until the Coordinator stops running.
