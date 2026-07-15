# Security policy

## Reporting a vulnerability

Use GitHub's private vulnerability reporting for this repository. Please include the affected version, minimal reproduction, impact, and whether untrusted link or fixture content is required. Do not open a public issue for a vulnerability before maintainers have had a reasonable opportunity to investigate.

## Supported version

Until a stable release exists, only the current default branch receives security fixes.

## Threat model

DeepLink Lab opens user-declared URLs on local test devices and reads user-declared local configuration files. Treat specifications and fixtures from untrusted parties as code-adjacent input:

- inspect links before running them;
- use dedicated Simulator/emulator instances without personal accounts or data;
- do not point probes at secrets;
- do not run third-party fixture build scripts without review;
- keep authenticated consumer applications out of test devices.

File probes reject absolute paths and parent traversal. The project intentionally does not support arbitrary shell-command probes, credential storage, account automation, or managed device enrollment.
