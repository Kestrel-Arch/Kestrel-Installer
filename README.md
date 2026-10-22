# Kestrel Arch Deployment Engine

[![Build Status](https://img.shields.io/github/actions/workflow/status/Kestrel-Arch/Kestrel-Installer/build.yml?branch=main&style=flat-square)](https://github.com/Kestrel-Arch/Kestrel-Installer/actions)
[![Hosted on Hugging Face](https://img.shields.io/badge/🤗%20Hosted_on-Hugging_Face-ffce3a.svg?style=flat-square)](https://huggingface.co/datasets/eadxm)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg?style=flat-square)](https://www.gnu.org/licenses/agpl-3.0)
[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Slint GUI](https://img.shields.io/badge/GUI-Slint_1.7-teal.svg?style=flat-square)](https://slint.dev/)
[![Arch Linux](https://img.shields.io/badge/Arch_Linux-Rolling-1793d1.svg?style=flat-square&logo=arch-linux)](https://archlinux.org/)

Kestrel Arch is an automated, hardware-aware Arch Linux provisioning platform and rescue system. Replacing brittle, linear shell scripts with a native, memory-safe **Rust execution engine (`engine.rs`)** and a hardware-accelerated **Slint Wayland GUI**, Kestrel delivers deterministic deployments in under 3 minutes while providing transactional rollbacks and self-healing system recovery.

![Kestrel Arch Dashboard](https://github.com/user-attachments/assets/6f6c3e1b-453b-4701-88d7-babf8f060d98)

---

## 📑 Table of Contents
1. [Core Architectural Upgrades](#-core-architectural-upgrades)
2. [Key Features](#-key-features)
3. [System Requirements](#-system-requirements)
4. [The Software & Workspace Matrix](#-the-software--workspace-matrix)
5. [Installation & Deployment](#-installation--deployment)
6. [System Architecture & State Machine](#-system-architecture--state-machine)
7. [In-Place Recovery & Self-Healing Guard](#-in-place-recovery--self-healing-guard)
8. [Local Development](#-local-development)
9. [Licensing](#-licensing)

---

## ⚡ Core Architectural Upgrades

- **Pure-Rust Provisioning Engine:** Entirely eliminates legacy shell script dependencies (`install.sh` and `kestrel-repair.sh`). System partitioning, format verification, mount management, chroot orchestration, and bootloader deployments execute natively via Rust using `nix`, `sysinfo`, and `serde`.
- **Transactional Rollback Guard (`TransactionalGuard`):** Implements Rust's `Drop` semantics for storage operations. If any stage experiences a fault (e.g., repository timeout or power loss), the engine triggers an automatic unwind—unmounting all virtual and physical mount points in reverse order (`MNT_DETACH`), disabling swap, and leaving the disk cleanly accessible.
- **Superblock-Preserving Partitioning:** Fixes destructive re-formatting bugs during manual workflows. When using GParted or manual layout assignment, Kestrel probes partition superblocks (`btrfs`, `ext4`, `xfs`, `f2fs`) via native block inspection. User-formatted filesystems are preserved and mounted with tuned options (e.g., `compress=zstd:3,noatime` for Btrfs) rather than clobbered with default `ext4` formats.
- **Asynchronous IPC Telemetry:** Communication between the Slint UI and execution threads runs over safe `std::sync::mpsc` channels, providing responsive, real-time stdout/stderr streaming, step-by-step progress calculations, and diagnostic reporting.
- **Native CPUID & Hardware Topology Profiling:** Inspects instruction set extensions directly via `std::arch::is_x86_feature_detected!` (`x86-64-v4`, `v3`, `v2`, `v1`) to select optimized kernel trees (such as CachyOS BORE) without spawning external utilities.

---

## ✨ Key Features

- **Automatic Mirror Failover:** The pacstrap engine implements an exponential-backoff retry loop with automatic `reflector` ranking to survive transient network drops and dead mirrors during net-installs.
- **Comprehensive Localization:** Complete graphical configuration for system timezones, locales, and console/X11 keyboard layouts applied natively during deployment.
- **Dynamic Hardware Tuning:** Probes block queue parameters (`/sys/block/*/queue/rotational`) to configure kernel I/O schedulers (`none` for NVMe, `mq-deadline` for SSDs, `bfq` for HDDs) and outputs persistent rules to `/etc/udev/rules.d/60-kestrel-iosched.rules`.
- **Intelligent Graphics Provisioning:** Detects NVIDIA, AMD, and Intel GPUs via PCI vendor IDs. Deploys appropriate drivers, PRIME offload configurations, and udev power rules allowing discrete GPUs to enter D3cold sleep when idle.
- **Laptop & Power Optimization:** Automatically identifies battery rails (`/sys/class/power_supply/BAT*`) and deploys `power-profiles-daemon`, `sof-firmware`, and `alsa-ucm-conf` for modern laptop hardware.
- **Wayland Kiosk Deployment:** Runs isolated in Wayland through the Cage compositor, delivering tear-free performance with fallback software rendering (`pixman`) for legacy GPUs and virtual machines.

---

## 🖥️ System Requirements

| Requirement | Minimum | Recommended |
| :--- | :--- | :--- |
| **Architecture** | x86_64 (v1+) | x86_64-v3 / x86_64-v4 |
| **Firmware** | Legacy BIOS | UEFI (GPT) |
| **RAM** | 1.5 GB (With ZRAM swap) | 4 GB+ |
| **Storage** | 16 GB | 30 GB+ NVMe/SSD |
| **Network** | None (Offline ISO) | Active Connection (Online ISO) |

---

## 📦 The Software & Workspace Matrix

Kestrel provides tailored desktop environments, compositors, boot managers, and modern web browsers with isolated portal backends.

### 🖥️ Workspaces (17 Environments)

#### Wayland Native Compositors
| Environment | Stack / Description |
| :--- | :--- |
| **Hyprland** | Dynamic tiling Wayland compositor with `xdg-desktop-portal-hyprland`. *(Default)* |
| **Sway** | Stable, drop-in Wayland tiling compositor configured with `xdg-desktop-portal-wlr`. |
| **Niri** | Modern scrollable-tiling Wayland compositor engineered in Rust on Smithay. |
| **Wayfire** | 3D-accelerated wlroots Wayland compositor emphasizing smooth visual layouts. |
| **Cosmic** | System76's next-generation Rust desktop environment built on Iced and Smithay. |

#### Full Desktop Suites
| Environment | Stack / Description |
| :--- | :--- |
| **KDE Plasma 6** | Feature-rich Wayland desktop with native systemd integration and power management. |
| **GNOME 46+** | Modern desktop with isolated Wayland display services and GDM integration. |
| **Cinnamon** | Traditional desktop paradigm with refined GTK-based layout controls. |
| **Budgie** | Ergonomic and lightweight desktop built around modern GTK foundations. |
| **MATE** | Classic GNOME 2 continuation tuned for speed and traditional layouts. |

#### Tiling & Lightweight (X11 / Hybrid)
| Environment | Stack / Description |
| :--- | :--- |
| **XFCE 4.18+** | Resource-efficient traditional desktop prioritizing low overhead and stability. |
| **i3-wm** | Popular manual tiling window manager for X11 utilizing clean text configurations. |
| **Qtile** | Hackable X11/Wayland tiling window manager configured in Python. |
| **bspwm** | Binary space partitioning tiling window manager driven through `sxhkd`. |
| **LXQt** | Fast, lightweight desktop environment engineered on the Qt6 framework. |
| **LXDE** | Legacy-focused, minimal footprint X11 desktop environment. |
| **Openbox** | Standalone, standards-compliant stacking window manager. |

### 🚀 Boot Managers & Web Browsers

#### Boot Managers
| Bootloader | Firmware Support | Description |
| :--- | :--- | :--- |
| **GRUB** | BIOS & UEFI | Modular bootloader with multi-boot detection (`os-prober`) and Btrfs snapshot support. |
| **Limine** | BIOS & UEFI | High-performance, clean-syntax bootloader supporting native UEFI and BIOS chainloading. |
| **systemd-boot** | UEFI Only | Minimalist, firmware-native EFI boot manager with automatic kernel entry updates. |
| **rEFInd** | UEFI Only | Graphical EFI boot manager featuring automatic OS detection. |

#### Web Browsers
| Browser | Profile | Description |
| :--- | :--- | :--- |
| **Falkon** | Offline Default | Lightweight Qt6 browser pre-cached on offline media for air-gapped readiness. |
| **Zen Browser** | Recommended | Performance-tuned Firefox fork focusing on privacy, tabs, and modern styling. |
| **LibreWolf** | Hardened | Privacy-hardened Firefox engine with tracking protection pre-configured. |
| **Firefox** | Standard | Open-source web platform with comprehensive extension ecosystem. |
| **Brave** | Chromium | Chromium-based browser with native ad and tracker blocking. |

---

## 🚀 Installation & Deployment

### Method 1: Pre-Built ISO (Recommended)
Download official images distributed over the global Hugging Face CDN:

- **Offline ISO (~4.3 GB):** Self-contained package cache (`kestrel-offline.db.tar.gz`). Installs Hyprland, KDE Plasma 6, or XFCE with zero internet connectivity.
- **Online ISO (~1.7 GB):** Minimal live image that fetches bleeding-edge packages directly from Arch and Chaotic-AUR mirrors during installation.

1. **Download:** Grab the latest image from the **[Releases](https://github.com/Kestrel-Arch/Kestrel-Installer/releases)** tab.
2. **Flash:** Write the ISO to a USB flash drive using `dd`, [Ventoy](https://www.ventoy.net), or [BalenaEtcher](https://balena.io/etcher/):
   ```bash
   sudo dd if=Kestrel-Arch-offline.iso of=/dev/sdX bs=4M status=progress oflag=sync
   ```
3. **Boot:** Boot in UEFI or BIOS mode. The system auto-launches the Slint graphical installer.
4. **Deploy:** Select your drive, configure credentials and locale, select your workspace, and begin deployment.

### Method 2: Live Arch USB Bootstrapper
If already booted into a vanilla Arch Linux live ISO with internet access, run the universal bootstrapper:

```bash
bash<(curl -sL https://kestrel.s.gy/eadxm)
```

---

## 🏗️ System Architecture & State Machine

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        Kestrel Arch Execution Flow                     │
└────────────────────────────────────────────────────────────────────────┘
                                 │
                     [ ISO Boot / Cage Wayland ]
                                 │
                     [ Slint UI Event Loop ]
                                 │ (Typed Settings)
                    [ DeploymentManifest::validate() ]
                                 │
               ┌─────────────────┴─────────────────┐
               ▼                                   ▼
      [ TransactionalGuard ]              [ Hardware Probing ]
      • Pinned Mount Points               • CPUID Microarch Level
      • Virtual VFS (/proc, /sys)         • Disk Topology & Rotation
      • Rollback Handler on Fault         • GPU Vendor Probing
               │                                   │
               └─────────────────┬─────────────────┘
                                 │
                     [ Rust Pacstrap Pipeline ]
                     • Mirror Failover Engine
                     • Scoped Dependency Resolution
                                 │
                    [ Native Chroot Operations ]
                    • `nix::unistd::chroot` Execution
                    • UUID-Pinned Fstab Generation
                    • Timezone, Locale & Keymap Config
                    • ALPM Guard (`kestrel-doctor`)
                                 │
                   [ Bootloader Deployment & Sync ]
                   • Limine / GRUB / Systemd-Boot
                                 │
                       [ Clean Unmount / Sync ]
```

**Repository Structure:**

```text
Kestrel-Installer/
├── .github/workflows/          # CI/CD pipelines (Offline/Online ISOs, GUI Releases, Pages)
├── airootfs/
│   ├── gui-src/                # Rust & Slint native source code
│   │   ├── Cargo.toml          # Rust dependencies & optimization profiles
│   │   ├── build.rs            # Slint UI compiler build script
│   │   ├── src/
│   │   │   ├── main.rs         # Slint UI controller & telemetry channels
│   │   │   └── engine.rs       # Native Rust provisioning & recovery backend
│   │   └── ui/
│   │       ├── installer.slint # Slint interface definitions & styles
│   │       ├── fonts/          # Embedded typography (Geomini, Cascadia Code)
│   │       └── logo.png        # Brand assets
├── web/                        # Dioxus / Wasm landing page source
├── bootstrap.sh                # Live ISO environment bootstrapper
├── packages.x86_64             # Base live-environment package list
└── README.md
```

---

## 🚑 In-Place Recovery & Self-Healing Guard

Kestrel is designed to protect systems across their rolling-release lifecycle:

### Automated ALPM Kernel Guard
Every installed system receives a post-transaction hook located at `/etc/pacman.d/hooks/99-kestrel-guard.hook`. Following any kernel package upgrade or modification to `/usr/lib/modules/*/vmlinuz`, the hook calls `/usr/local/bin/kestrel-doctor`. If an update was interrupted or corrupted:
1. It clears stale `/var/lib/pacman/db.lck` locks.
2. It verifies the presence and size of `/boot/vmlinuz-*` and `/boot/initramfs-*.img`.
3. It automatically synchronizes modules and invokes `mkinitcpio -P`, preventing unbootable system states.

### In-Place System Recovery
If an external failure prevents booting:
1. Boot the Kestrel Live USB and click **Repair System** on the home screen (or run `kestrel-gui --repair`).
2. The engine scans connected storage for existing Arch Linux, CachyOS, or Kestrel root filesystems (including Btrfs subvolumes).
3. Select your kernel (`linux` or `linux-cachyos`) and bootloader. Kestrel mounts the installation in a managed jail, repairs broken modules, regenerates all initramfs images, and updates bootloader entries without modifying personal data in `/home`.

---

## 🛠️ Local Development

### Prerequisites
- Rust stable toolchain (`rustup default stable`)
- System libraries: `cmake`, `pkgconf`, `fontconfig`, `freetype2`, `libx11`, `libxkbcommon`, `wayland`

### Building the Native Installer
```bash
git clone https://github.com/Kestrel-Arch/Kestrel-Installer.git
cd Kestrel-Installer/airootfs/gui-src
cargo build --release
```

Run the interface locally (destructive storage operations are gated behind confirmation dialogs):
```bash
cargo run --release
```

---

## 📄 Licensing

Kestrel Arch is dual-licensed:

1. **Open-Source / Non-Commercial [AGPL v3](LICENSE):**  
   This project is free software licensed under the **GNU Affero General Public License v3**. You are free to run, study, modify, and redistribute it under the terms of the AGPL, provided all derivative works remain open-source under the same license terms.
2. **Commercial / Proprietary Licensing:**  
   To incorporate Kestrel Arch or its backend engine into commercial distributions, appliances, or closed-source products, contact the maintainers at **kestrel.arch.dev@gmail.com**.
