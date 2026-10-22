use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};

fn is_pacman_running() -> bool {
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.filter_map(|e| e.ok()) {
            let comm_path = entry.path().join("comm");
            if let Ok(comm) = fs::read_to_string(comm_path) {
                if comm.trim() == "pacman" {
                    return true;
                }
            }
        }
    }
    false
}

fn run_doctor() -> Result<(), String> {
    println!("==> [Kestrel Doctor] Auditing package manager & boot integrity...");

    // 1. Clear stale pacman db.lck if no pacman process is active in /proc
    let lock_path = Path::new("/var/lib/pacman/db.lck");
    if lock_path.exists() && !is_pacman_running() {
        println!("[Kestrel Doctor] Removing stale /var/lib/pacman/db.lck...");
        let _ = fs::remove_file(lock_path);
    }

    // 2. Scan all installed kernel module trees in /usr/lib/modules/*
    let modules_dir = Path::new("/usr/lib/modules");
    if let Ok(entries) = fs::read_dir(modules_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let pkgbase_file = path.join("pkgbase");
            if let Ok(pkgbase_raw) = fs::read_to_string(&pkgbase_file) {
                let pkgbase = pkgbase_raw.trim();
                if pkgbase.is_empty() {
                    continue;
                }

                let vmlinuz_boot = PathBuf::from(format!("/boot/vmlinuz-{}", pkgbase));
                let initramfs_boot = PathBuf::from(format!("/boot/initramfs-{}.img", pkgbase));

                let vmlinuz_missing = !vmlinuz_boot.exists()
                || fs::metadata(&vmlinuz_boot).map(|m| m.len() == 0).unwrap_or(true);
                let initramfs_missing = !initramfs_boot.exists()
                || fs::metadata(&initramfs_boot).map(|m| m.len() == 0).unwrap_or(true);

                if vmlinuz_missing || initramfs_missing {
                    println!(
                        "[Kestrel Doctor] Detected missing or desynchronized boot image for '{}'. Healing...",
                        pkgbase
                    );

                    let src_vmlinuz = path.join("vmlinuz");
                    if src_vmlinuz.exists() {
                        fs::copy(&src_vmlinuz, &vmlinuz_boot).map_err(|e| {
                            format!(
                                "Failed copying {} to {}: {}",
                                src_vmlinuz.display(),
                                    vmlinuz_boot.display(),
                                    e
                            )
                        })?;
                    }

                    let status = Command::new("mkinitcpio")
                    .args(["-p", pkgbase])
                    .status()
                    .map_err(|e| format!("Failed spawning mkinitcpio: {}", e))?;

                    if !status.success() {
                        return Err(format!("mkinitcpio exited with error for preset '{}'", pkgbase));
                    }
                }
            }
        }
    }

    // 3. Synchronize filesystem buffers
    let _ = Command::new("sync").status();
    println!("==> [Kestrel Doctor] Kernel & initramfs boot integrity verified.");
    Ok(())
}

fn main() {
    if let Err(err) = run_doctor() {
        eprintln!("[Kestrel Doctor CRITICAL] {}", err);
        exit(1);
    }
}
