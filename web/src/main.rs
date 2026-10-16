use dioxus::prelude::*;

fn main() {
    launch(App);
}

#[component]
fn App() -> Element {
    let mut active_de_filter = use_signal(|| "all");
    let mut show_Partition_modal = use_signal(|| false);

    rsx! {
        div { class: "min-h-screen bg-[#020617] text-gray-200 flex flex-col justify-between relative",

            // Subtle Ambient Background Glow
            div { class: "pointer-events-none fixed top-0 left-1/2 -translate-x-1/2 w-[900px] h-[450px] bg-cyan-500/10 blur-[140px] rounded-full z-0" }

            // ==========================================
            // TOP NAVIGATION BAR
            // ==========================================
            header { class: "border-b border-slate-800/80 bg-[#020617]/85 backdrop-blur-md sticky top-0 z-40",
                div { class: "max-w-6xl mx-auto px-6 h-16 flex items-center justify-between",
                    div { class: "flex items-center gap-3",
                        img {
                            class: "w-8 h-8 object-contain drop-shadow-[0_0_10px_rgba(6,182,212,0.5)]",
                            src: "logo.png",
                            alt: "Kestrel Arch Logo"
                        }
                        span { class: "font-bold text-white tracking-widest text-sm", "KESTREL ARCH" }
                        span { class: "text-[11px] px-2.5 py-0.5 rounded-full bg-cyan-950/90 border border-cyan-500/40 text-cyan-300 font-mono font-medium",
                            "ROLLING • RUST + SLINT"
                        }
                    }

                    nav { class: "hidden md:flex items-center gap-7 text-sm font-medium text-gray-400",
                        a { class: "hover:text-cyan-300 transition-colors", href: "#recovery", "Recovery Engine" }
                        a { class: "hover:text-cyan-300 transition-colors", href: "#architecture", "Architecture" }
                        a { class: "hover:text-cyan-300 transition-colors", href: "#environments", "Desktops (17)" }
                        a {
                            class: "hover:text-white transition-colors",
                            href: "https://github.com/Kestrel-Arch/Kestrel-Installer",
                            target: "_blank",
                            "GitHub"
                        }
                        a {
                            class: "px-3.5 py-1.5 rounded-lg bg-cyan-500/10 border border-cyan-500/40 text-cyan-300 hover:bg-cyan-500 hover:text-black font-semibold transition-all",
                            href: "https://github.com/Kestrel-Arch/Kestrel-Installer/releases",
                            target: "_blank",
                            "Releases →"
                        }
                    }
                }
            }

            // ==========================================
            // MAIN CONTENT CONTAINER
            // ==========================================
            main { class: "max-w-6xl mx-auto px-6 pt-14 pb-24 space-y-28 relative z-10",

                // ------------------------------------------
                // HERO SECTION (PLEXO-INSPIRED SPLIT LAYOUT)
                // ------------------------------------------
                section { class: "grid grid-cols-1 lg:grid-cols-12 gap-12 items-start",

                    // Left Column: Headline, Description, Dual ISO CTAs
                    div { class: "lg:col-span-7 space-y-7 pt-2",
                        div { class: "inline-flex items-center gap-2.5 px-3 py-1 rounded-full bg-[#040D14] border border-cyan-500/40 text-xs font-mono text-cyan-300 tracking-wide",
                            span { class: "w-2 h-2 rounded-full bg-cyan-400 animate-ping" }
                            "DEPLOYMENT & IN-PLACE RECOVERY ENGINE"
                        }

                        h1 { class: "text-5xl sm:text-6xl font-extrabold text-white tracking-tight leading-[1.06]",
                            "One installer."
                            br {}
                            span { class: "text-transparent bg-clip-text bg-gradient-to-r from-teal-400 via-cyan-400 to-cyan-200",
                                "Every scenario."
                            }
                        }

                        p { class: "text-lg text-gray-400 max-w-xl leading-relaxed",
                            "Kestrel Arch transforms a multi-hour command-line installation into a 3-minute graphical deployment. Built with a memory-safe "
                            span { class: "text-gray-200 font-semibold", "Rust + Slint GUI" }
                            ", CachyOS BORE kernel support, and a non-destructive rescue engine that fixes broken bootloaders without touching your files."
                        }

                        p { class: "text-sm font-medium text-cyan-300/90",
                            "Air-gapped Offline cache, bleeding-edge Online net-install, UEFI & Legacy BIOS."
                        }

                        // Primary Download Buttons
                        div { class: "flex flex-col sm:flex-row items-stretch sm:items-center gap-4 pt-2",
                            a {
                                class: "px-6 py-4 rounded-xl bg-gradient-to-r from-teal-500 to-cyan-500 text-black hover:brightness-110 transition-all kestrel-glow flex items-center justify-center gap-3.5",
                                href: "https://github.com/Kestrel-Arch/Kestrel-Installer/releases/tag/latest-iso",
                                target: "_blank",
                                span { class: "text-xl font-bold", "↓" }
                                div { class: "text-left",
                                    div { class: "text-sm font-extrabold leading-tight", "Download Offline ISO" }
                                    div { class: "text-[11px] font-mono font-semibold opacity-80", "Air-Gapped • Hyprland / Plasma 6 / XFCE" }
                                }
                            }

                            a {
                                class: "px-6 py-4 rounded-xl bg-[#040D14] border border-cyan-500/40 hover:border-cyan-400 hover:bg-cyan-950/30 text-gray-100 transition-all flex items-center justify-center gap-3.5",
                                href: "https://github.com/Kestrel-Arch/Kestrel-Installer/releases/tag/latest-online-iso",
                                target: "_blank",
                                span { class: "text-xl font-bold text-cyan-400", "↓" }
                                div { class: "text-left",
                                    div { class: "text-sm font-bold leading-tight text-white", "Download Online ISO" }
                                    div { class: "text-[11px] font-mono text-gray-400", "Net-Installer • All 17 Desktops & 5 Browsers" }
                                }
                            }
                        }

                        // Quick metadata links
                        div { class: "flex flex-wrap items-center gap-4 text-xs font-mono text-gray-500 pt-1",
                            span { "x86_64 Architecture" }
                            span { "•" }
                            span { "Hosted on Hugging Face CDN" }
                            span { "•" }
                            a {
                                class: "text-cyan-400 hover:underline",
                                href: "https://github.com/Kestrel-Arch/Kestrel-Installer/releases",
                                target: "_blank",
                                "All downloads & GUI binaries →"
                            }
                        }

                        // Callout Banner (Like Plexo's bottom notice bar)
                        div { class: "p-4 rounded-xl bg-[#040D14]/90 border border-cyan-500/30 flex items-center justify-between gap-4 max-w-xl",
                            div { class: "flex items-center gap-3 text-xs text-gray-300",
                                span { class: "text-cyan-400 font-bold text-base", "⚡" }
                                span { "Includes live GParted visual partitioning, Wi-Fi station scanner, and ZRAM matrix." }
                            }
                            button {
                                class: "text-xs font-mono font-semibold text-cyan-400 hover:text-cyan-300 shrink-0 cursor-pointer",
                                onclick: move |_| show_Partition_modal.set(true),
                                "See specs"
                            }
                        }
                    }

                    // Right Column: "What's New" Release & Capabilities Card
                    div { class: "lg:col-span-5",
                        div { class: "bg-[#040D14] rounded-2xl p-6 border border-cyan-500/30 kestrel-card-glow space-y-6",
                            div { class: "flex items-center justify-between border-b border-slate-800 pb-4",
                                div { class: "flex items-center gap-2.5",
                                    span { class: "text-white font-bold text-base", "What's new" }
                                    span { class: "text-xs font-mono text-cyan-400", "rolling-release" }
                                }
                                span { class: "text-xs font-mono text-gray-500", "Daily 12:00 GMT" }
                            }

                            div { class: "space-y-5",
                                HighlightItem {
                                    badge: "RECOVERY",
                                    badge_color: "bg-emerald-950 text-emerald-400 border-emerald-500/30",
                                    title: "In-Place System & Bootloader Repair",
                                    desc: "Mounts damaged Arch or CachyOS partitions, reinstalls kernels, rebuilds initramfs via mkinitcpio, and restores bootloaders without wiping /home."
                                }
                                HighlightItem {
                                    badge: "FIRMWARE",
                                    badge_color: "bg-cyan-950 text-cyan-400 border-cyan-500/30",
                                    title: "Hardware-Gated UEFI / BIOS Logic",
                                    desc: "Probes /sys/firmware/efi in real time. Automatically hides UEFI-only bootloaders (systemd-boot, rEFInd) and ESP prompts on Legacy BIOS machines."
                                }
                                HighlightItem {
                                    badge: "KERNEL",
                                    badge_color: "bg-purple-950 text-purple-300 border-purple-500/30",
                                    title: "CachyOS BORE Scheduler Auto-Detection",
                                    desc: "Detects /etc/cachyos-release during system recovery and automatically enforces linux-cachyos and its matching headers."
                                }
                            }

                            // Active Rolling Channels (Like Plexo's "Earlier Versions" table)
                            div { class: "pt-4 border-t border-slate-800/80 space-y-3",
                                div { class: "text-[11px] font-mono uppercase tracking-wider text-gray-500 font-semibold",
                                    "ACTIVE ROLLING CHANNELS"
                                }
                                div { class: "space-y-2 text-xs font-mono",
                                    ReleaseRow {
                                        tag: "latest-iso",
                                        desc: "Offline Air-Gapped ISO (HF CDN)",
                                        url: "https://github.com/Kestrel-Arch/Kestrel-Installer/releases/tag/latest-iso"
                                    }
                                    ReleaseRow {
                                        tag: "latest-online-iso",
                                        desc: "Online Net-Installer ISO (HF CDN)",
                                        url: "https://github.com/Kestrel-Arch/Kestrel-Installer/releases/tag/latest-online-iso"
                                    }
                                    ReleaseRow {
                                        tag: "latest-gui",
                                        desc: "Standalone Slint GUI Binary",
                                        url: "https://github.com/Kestrel-Arch/Kestrel-Installer/releases/tag/latest-gui"
                                    }
                                }
                                div { class: "pt-2",
                                    a {
                                        class: "text-xs font-semibold text-cyan-400 hover:text-cyan-300 inline-flex items-center gap-1",
                                        href: "https://github.com/Kestrel-Arch/Kestrel-Installer/releases",
                                        target: "_blank",
                                        "All releases on GitHub →"
                                    }
                                }
                            }
                        }
                    }
                }

                // ------------------------------------------
                // SECTION 2: IN-PLACE RECOVERY ENGINE
                // ------------------------------------------
                section { id: "recovery", class: "space-y-8",
                    div { class: "space-y-2",
                        div { class: "text-xs font-mono text-cyan-400 uppercase tracking-widest", "RESCUE & RESTORE" }
                        h2 { class: "text-3xl font-bold text-white", "Broken update? Recover in 60 seconds." }
                        p { class: "text-gray-400 max-w-2xl text-sm",
                            "Instead of manually chrooting from a terminal, launch Kestrel's graphical Automated System Recovery screen directly from the live ISO."
                        }
                    }

                    div { class: "grid grid-cols-1 md:grid-cols-3 gap-6",
                        StepCard {
                            step: "01",
                            title: "Automatic Root Probing",
                            desc: "Scans all ext4, btrfs, xfs, and f2fs volumes read-only to locate existing Arch Linux, CachyOS, or Kestrel installations alongside FAT32 EFI partitions."
                        }
                        StepCard {
                            step: "02",
                            title: "Smart Kernel Enforcement",
                            desc: "If a CachyOS installation is detected, Kestrel locks in linux-cachyos and its BORE scheduler headers. Standard Arch roots let you choose between linux and linux-cachyos."
                        }
                        StepCard {
                            step: "03",
                            title: "Bootloader Reconstruction",
                            desc: "Re-runs mkinitcpio -P inside an isolated arch-chroot and deploys GRUB, Limine, systemd-boot, or rEFInd with live stdout/stderr terminal streaming."
                        }
                    }
                }

                // ------------------------------------------
                // SECTION 3: CORE ARCHITECTURE MATRIX
                // ------------------------------------------
                section { id: "architecture", class: "space-y-8",
                    div { class: "space-y-2",
                        div { class: "text-xs font-mono text-cyan-400 uppercase tracking-widest", "UNDER THE HOOD" }
                        h2 { class: "text-3xl font-bold text-white", "Engineered for zero-friction deployment." }
                    }

                    div { class: "grid grid-cols-1 md:grid-cols-2 gap-6",
                        ArchCard {
                            title: "3 Partitioning Strategies",
                            subtitle: "Wipe Disk • Replace Partition • Manual GParted",
                            desc: "Choose automatic full-disk formatting (ext4 or Btrfs), single-partition replacement that leaves neighboring OS partitions intact, or launch GParted visually with real-time partition bar color-coding."
                        }
                        ArchCard {
                            title: "Hyper-Performance Matrix",
                            subtitle: "ZRAM • scx-scheds • EarlyOOM • Chaotic-AUR",
                            desc: "Pre-configured with Chaotic-AUR keyrings, compressed RAM swap via zram-generator, EarlyOOM protection, and automatic VM/NVIDIA Wayland pixman renderer fallbacks."
                        }
                        ArchCard {
                            title: "4 Bootloaders (Firmware-Filtered)",
                            subtitle: "GRUB • Limine • systemd-boot • rEFInd",
                            desc: "Universal support for GRUB and Limine across both GPT/UEFI and MBR/BIOS, plus minimalist systemd-boot and graphical rEFInd on UEFI hardware."
                        }
                        ArchCard {
                            title: "5 Privacy & Speed Browsers",
                            subtitle: "Zen • LibreWolf • Firefox • Brave • Falkon",
                            desc: "Select your primary browser during setup. Offline builds default to the lightweight Qt-based Falkon browser for instant air-gapped readiness."
                        }
                    }
                }

                // ------------------------------------------
                // SECTION 4: 17 DESKTOP ENVIRONMENTS & WMS
                // ------------------------------------------
                section { id: "environments", class: "space-y-6",
                    div { class: "flex flex-col sm:flex-row sm:items-end justify-between gap-4",
                        div { class: "space-y-2",
                            div { class: "text-xs font-mono text-cyan-400 uppercase tracking-widest", "WORKSPACES" }
                            h2 { class: "text-3xl font-bold text-white", "17 Desktop Environments & Window Managers" }
                        }

                        // Filter Buttons
                        div { class: "flex items-center gap-2 bg-[#040D14] p-1 rounded-xl border border-slate-800 text-xs font-mono",
                            button {
                                class: if *active_de_filter.read() == "all" { "px-3 py-1.5 rounded-lg bg-cyan-500 text-black font-bold cursor-pointer" } else { "px-3 py-1.5 rounded-lg text-gray-400 hover:text-white cursor-pointer" },
                                onclick: move |_| active_de_filter.set("all"),
                                "All (17)"
                            }
                            button {
                                class: if *active_de_filter.read() == "offline" { "px-3 py-1.5 rounded-lg bg-cyan-500 text-black font-bold cursor-pointer" } else { "px-3 py-1.5 rounded-lg text-gray-400 hover:text-white cursor-pointer" },
                                onclick: move |_| active_de_filter.set("offline"),
                                "Offline + Online (3)"
                            }
                            button {
                                class: if *active_de_filter.read() == "tiling" { "px-3 py-1.5 rounded-lg bg-cyan-500 text-black font-bold cursor-pointer" } else { "px-3 py-1.5 rounded-lg text-gray-400 hover:text-white cursor-pointer" },
                                onclick: move |_| active_de_filter.set("tiling"),
                                "Tiling WMs (8)"
                            }
                        }
                    }

                    div { class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4",
                        for (name, category, badge, desc) in [
                            ("1. Hyprland", "offline_tiling", "OFFLINE + ONLINE", "Visually pleasing dynamic tiling Wayland compositor."),
                            ("2. KDE Plasma 6", "offline_de", "OFFLINE + ONLINE", "Comprehensive, flexible next-gen Wayland desktop environment."),
                            ("3. XFCE 4.18+", "offline_de", "OFFLINE + ONLINE", "Modernized, lightweight, and rock-solid traditional desktop."),
                            ("4. GNOME 46+", "online_de", "ONLINE ISO", "User-friendly environment with a modern, fluid layout."),
                            ("5. Sway", "online_tiling", "ONLINE ISO", "Tiling Wayland compositor; drop-in i3 replacement."),
                            ("6. i3-wm", "online_tiling", "ONLINE ISO", "Popular X11 tiling manager leveraging clean text configs."),
                            ("7. Cinnamon", "online_de", "ONLINE ISO", "Traditional desktop paradigm balancing advanced features."),
                            ("8. Niri", "online_tiling", "ONLINE ISO", "Scrollable tiling Wayland compositor optimizing fluid grids."),
                            ("9. Qtile", "online_tiling", "ONLINE ISO", "Configurable X11/Wayland environment scripted in Python."),
                            ("10. Wayfire", "online_tiling", "ONLINE ISO", "Wlroots Wayland engine mixing 3D effects and performance."),
                            ("11. bspwm", "online_tiling", "ONLINE ISO", "Binary space partitioning X11 architecture."),
                            ("12. Budgie", "online_de", "ONLINE ISO", "Clean and elegant GTK interface prioritizing ergonomics."),
                            ("13. Cosmic Epoch", "online_tiling", "ONLINE ISO • RUST", "System76's modern Rust-based workspace built for speed."),
                            ("14. LXDE", "online_de", "ONLINE ISO", "Fast, ultra-lightweight X11 environment for legacy hardware."),
                            ("15. LXQt", "online_de", "ONLINE ISO", "Blazing fast lightweight environment engineered on Qt."),
                            ("16. Mate", "online_de", "ONLINE ISO", "Classic GNOME 2 fork tracking legacy desktop layouts."),
                            ("17. Openbox", "online_tiling", "ONLINE ISO", "Highly customizable stacking X11 window manager."),
                        ] {
                            if *active_de_filter.read() == "all"
                                || (*active_de_filter.read() == "offline" && category.starts_with("offline"))
                                || (*active_de_filter.read() == "tiling" && category.ends_with("tiling"))
                            {
                                div { class: "p-4 rounded-xl bg-[#040D14] border border-slate-800/90 hover:border-cyan-500/40 transition-all space-y-1.5",
                                    div { class: "flex items-center justify-between gap-2",
                                        span { class: "font-bold text-white text-sm", "{name}" }
                                        span {
                                            class: if category.starts_with("offline") {
                                                "text-[10px] font-mono px-2 py-0.5 rounded bg-cyan-950 text-cyan-300 border border-cyan-500/30 font-semibold"
                                            } else {
                                                "text-[10px] font-mono px-2 py-0.5 rounded bg-slate-900 text-gray-400 border border-slate-700"
                                            },
                                            "{badge}"
                                        }
                                    }
                                    p { class: "text-xs text-gray-400 leading-relaxed", "{desc}" }
                                }
                            }
                        }
                    }
                }
            }

            // ==========================================
            // FOOTER
            // ==========================================
            footer { class: "border-t border-slate-900 bg-[#020617] py-10 px-6 relative z-10",
                div { class: "max-w-6xl mx-auto flex flex-col sm:flex-row items-center justify-between gap-4 text-xs text-gray-500",
                    div { class: "flex items-center gap-3",
                        img { class: "w-5 h-5 object-contain opacity-80", src: "logo.png" }
                        span { class: "font-bold text-gray-300", "Kestrel Arch Deployment Engine" }
                        span { "•" }
                        span { "Licensed under AGPLv3" }
                    }
                    div { class: "flex items-center gap-6 font-mono",
                        a {
                            class: "hover:text-cyan-400 transition-colors",
                            href: "https://github.com/Kestrel-Arch/Kestrel-Installer",
                            target: "_blank",
                            "Source Code"
                        }
                        a {
                            class: "hover:text-cyan-400 transition-colors",
                            href: "https://github.com/Kestrel-Arch/Kestrel-Installer/releases",
                            target: "_blank",
                            "ISO Releases"
                        }
                        a {
                            class: "hover:text-cyan-400 transition-colors",
                            href: "https://github.com/Kestrel-Arch/Kestrel-Installer/actions",
                            target: "_blank",
                            "CI Status"
                        }
                    }
                }
            }

            // ==========================================
            // TECHNICAL SPECS MODAL
            // ==========================================
            if *show_Partition_modal.read() {
                div { class: "fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm",
                    div { class: "bg-[#040D14] border border-cyan-500/50 rounded-2xl max-w-lg w-full p-6 space-y-4 kestrel-glow",
                        div { class: "flex items-center justify-between border-b border-slate-800 pb-3",
                            h3 { class: "text-lg font-bold text-white", "Kestrel Arch Technical Specifications" }
                            button {
                                class: "text-gray-400 hover:text-white font-mono text-sm cursor-pointer",
                                onclick: move |_| show_Partition_modal.set(false),
                                "✕"
                            }
                        }
                        div { class: "space-y-2.5 text-xs font-mono text-gray-300",
                            div { class: "p-3 rounded-lg bg-slate-950 border border-slate-800",
                                span { class: "text-cyan-400 font-bold", "Live Environment: " }
                                "Cage Wayland Kiosk + Pixman VM/NVIDIA Fallback"
                            }
                            div { class: "p-3 rounded-lg bg-slate-950 border border-slate-800",
                                span { class: "text-cyan-400 font-bold", "Frontend GUI: " }
                                "Rust + Slint UI with real-time lsblk visualizer"
                            }
                            div { class: "p-3 rounded-lg bg-slate-950 border border-slate-800",
                                span { class: "text-cyan-400 font-bold", "Backend Engines: " }
                                "install.sh & kestrel-repair.sh (Non-Interactive IPC)"
                            }
                            div { class: "p-3 rounded-lg bg-slate-950 border border-slate-800",
                                span { class: "text-cyan-400 font-bold", "Compression & Repo: " }
                                "Zstandard (zstd) SquashFS + Chaotic-AUR pre-indexed"
                            }
                        }
                        div { class: "pt-2 flex justify-end",
                            button {
                                class: "px-4 py-2 rounded-lg bg-cyan-500 text-black font-bold text-xs hover:bg-cyan-400 transition-colors cursor-pointer",
                                onclick: move |_| show_Partition_modal.set(false),
                                "Close"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn HighlightItem(
    badge: &'static str,
    badge_color: &'static str,
    title: &'static str,
    desc: &'static str,
) -> Element {
    rsx! {
        div { class: "space-y-1.5",
            div { class: "flex items-center gap-2.5",
                span { class: "text-[10px] font-mono px-2 py-0.5 rounded border font-bold {badge_color}",
                    "{badge}"
                }
                span { class: "text-sm font-bold text-white", "{title}" }
            }
            p { class: "text-xs text-gray-400 leading-relaxed", "{desc}" }
        }
    }
}

#[component]
fn ReleaseRow(tag: &'static str, desc: &'static str, url: &'static str) -> Element {
    rsx! {
        a {
            class: "flex items-center justify-between py-1.5 px-2.5 rounded-lg hover:bg-slate-900/80 transition-colors group",
            href: "{url}",
            target: "_blank",
            span { class: "text-cyan-400 font-semibold group-hover:underline", "{tag}" }
            span { class: "text-gray-500 text-[11px]", "{desc}" }
        }
    }
}

#[component]
fn StepCard(step: &'static str, title: &'static str, desc: &'static str) -> Element {
    rsx! {
        div { class: "p-6 rounded-2xl bg-[#040D14] border border-cyan-500/20 hover:border-cyan-500/50 transition-all space-y-3",
            div { class: "text-xs font-mono font-bold text-cyan-400 px-2.5 py-1 rounded bg-cyan-950/60 border border-cyan-500/30 inline-block",
                "STEP {step}"
            }
            h3 { class: "text-lg font-bold text-white", "{title}" }
            p { class: "text-xs text-gray-400 leading-relaxed", "{desc}" }
        }
    }
}

#[component]
fn ArchCard(title: &'static str, subtitle: &'static str, desc: &'static str) -> Element {
    rsx! {
        div { class: "p-6 rounded-2xl bg-[#040D14] border border-slate-800 hover:border-cyan-500/40 transition-all space-y-2.5",
            h3 { class: "text-lg font-bold text-white", "{title}" }
            div { class: "text-xs font-mono text-cyan-400 font-medium", "{subtitle}" }
            p { class: "text-xs text-gray-400 leading-relaxed pt-1", "{desc}" }
        }
    }
}
