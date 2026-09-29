use dioxus::prelude::*;

fn main() {
    launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        // Tailwind CDN inject
        link { rel: "stylesheet", href: "https://cdn.jsdelivr.net/npm/tailwindcss@2.2.19/dist/tailwind.min.css" }
        style {
            "body {{ background-color: #030712; color: #cbd5e1; font-family: ui-sans-serif, system-ui, sans-serif; }}"
        }

        // Top Navigation
        header { class: "border-b border-gray-900 bg-gray-950 px-6 py-4 sticky top-0 z-50",
            div { class: "max-w-6xl mx-auto flex items-center justify-between",
                div { class: "flex items-center gap-3",
                    div { class: "w-8 h-8 rounded-lg bg-cyan-950 border border-cyan-500/40 flex items-center justify-center font-bold text-cyan-400 text-sm",
                        "K"
                    }
                    span { class: "font-bold text-white tracking-wider text-sm", "KESTREL ARCH" }
                    span { class: "text-xs px-2 py-0.5 rounded-full bg-cyan-950 border border-cyan-500/30 text-cyan-300 font-mono",
                        "Rust + Wasm"
                    }
                }
                nav { class: "flex items-center gap-6 text-sm",
                    a { 
                        class: "text-gray-400 hover:text-white transition-colors", 
                        href: "https://github.com/Kestrel-Arch/Kestrel-Installer", 
                        "GitHub" 
                    }
                    a { 
                        class: "text-cyan-400 hover:text-cyan-300 font-medium", 
                        href: "https://github.com/Kestrel-Arch/Kestrel-Installer/releases", 
                        "Releases →" 
                    }
                }
            }
        }

        // Hero Section
        main { class: "max-w-6xl mx-auto px-6 pt-16 pb-24",
            div { class: "grid grid-cols-1 lg:grid-cols-12 gap-12 items-start",
                
                // Left Column
                div { class: "lg:col-span-7 space-y-6",
                    div { class: "inline-flex items-center gap-2 px-3 py-1 rounded-full bg-cyan-950 border border-cyan-500 text-xs font-mono text-cyan-400",
                        span { class: "w-2 h-2 rounded-full bg-cyan-400" }
                        "HARDWARE-AWARE & BULLETPROOF"
                    }

                    h1 { class: "text-5xl sm:text-6xl font-extrabold text-white tracking-tight leading-tight",
                        "Effortless Arch."
                        br {}
                        span { class: "text-cyan-400", "Instant recovery." }
                    }

                    p { class: "text-lg text-gray-400 max-w-xl",
                        "Kestrel Arch deploys in under 3 minutes via a memory-safe Slint GUI and features an in-place rescue engine to fix broken kernels without wiping /home."
                    }

                    div { class: "flex flex-wrap gap-4 pt-4",
                        a {
                            class: "px-6 py-3.5 rounded-xl bg-cyan-500 text-black font-semibold hover:bg-cyan-400 transition-all shadow-lg text-sm",
                            href: "https://github.com/Kestrel-Arch/Kestrel-Installer/releases/latest",
                            "Download Kestrel ISO"
                        }
                        a {
                            class: "px-6 py-3.5 rounded-xl bg-gray-900 border border-gray-800 text-gray-200 font-medium hover:border-cyan-500 text-sm transition-all",
                            href: "#specs",
                            "Architecture Specs"
                        }
                    }
                }

                // Right Column / Feature Cards
                div { class: "lg:col-span-5 bg-gray-950 rounded-2xl p-6 border border-cyan-500/20 shadow-2xl space-y-6",
                    h2 { class: "text-white font-bold text-base border-b border-gray-800 pb-3",
                        "Core Systems Overview"
                    }
                    FeatureItem {
                        badge: "RECOVERY",
                        title: "In-Place Clean Root Reinstall",
                        description: "Detects damaged Arch or CachyOS installs and repairs modules while preserving personal data.",
                    }
                    FeatureItem {
                        badge: "FIRMWARE",
                        title: "Dynamic Bootloader Gating",
                        description: "Probes host NVRAM; strips UEFI-only bootloaders (systemd-boot/rEFInd) on legacy BIOS machines.",
                    }
                    FeatureItem {
                        badge: "OPTIMIZED",
                        title: "CachyOS / BORE Ready",
                        description: "Direct scheduler integration with custom kernel-level hooks.",
                    }
                }
            }
        }
    }
}

#[component]
fn FeatureItem(badge: &'static str, title: &'static str, description: &'static str) -> Element {
    rsx! {
        div { class: "space-y-1",
            div { class: "flex items-center gap-2",
                span { class: "text-[10px] font-mono px-1.5 py-0.5 rounded bg-cyan-950 text-cyan-400 font-bold",
                    "{badge}"
                }
                span { class: "text-sm font-semibold text-gray-200", "{title}" }
            }
            p { class: "text-xs text-gray-400 leading-relaxed", "{description}" }
        }
    }
}