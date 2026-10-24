# Security Policy

## Supported Versions

Since Kestrel Arch is a rolling-release distribution built around up-to-date Arch Linux repositories and custom deployment components, only the latest state of the main branch and current ISO releases are actively supported with security updates.

| Version | Supported          |
| ------- | ------------------ |
| Latest (Main / Active ISO) | :white_check_mark: |
| Older / EOL Releases | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability within Kestrel Arch — such as issues regarding the native Rust provisioning engine (`engine.rs`), mount namespace / `chroot` privilege handling, `kestrel-doctor` ALPM hooks, bootloader configurations (GRUB, Limine, systemd-boot, rEFInd), or the Rust/Slint GUI — please report it responsibly:

* **Private Disclosure:** Please **do not** open a public issue or discuss the vulnerability in public forums before it has been addressed.
* **How to Report:** Open a private **Security Advisory** directly on the GitHub repository, or reach out to the maintainer privately at **kestrel.arch.dev@gmail.com**.
* **What to Expect:** You can expect an initial response or acknowledgment of your report within **48 to 72 hours**, followed by updates on whether the vulnerability is accepted and how/when a patch will be deployed.