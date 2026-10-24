# Contributing to Kestrel Arch

Thank you for your interest in contributing to Kestrel Arch! Whether you are fixing a bug in the native Rust provisioning engine, improving the Slint Wayland GUI, or updating offline cache logic, your help is appreciated.

## How to Contribute
1. **Fork the Repository** and clone your fork locally.
2. **Create a Branch** for your feature or bug fix (`git checkout -b feature/amazing-feature`).
3. **Commit Your Changes** (`git commit -m 'Add some amazing feature'`).
4. **Push to the Branch** (`git push origin feature/amazing-feature`).
5. **Open a Pull Request** against the `main` branch.

## Project Structure
* `airootfs/gui-src/src/engine.rs`: The native Rust deployment and recovery engine handling partitioning, superblock inspection, chroot isolation (`TransactionalGuard`), pacstrap streaming, and bootloaders.
* `airootfs/gui-src/src/bin/kestrel-doctor.rs`: The standalone Rust ALPM post-transaction kernel & initramfs integrity guard.
* `airootfs/gui-src/ui/installer.slint`: The Slint Wayland UI definitions, components, and vector assets (`logo.svg`).
* `web/`: The Dioxus WebAssembly landing page source code.
* `.github/workflows/`: The automation pipelines handling ISO compilation, standalone binary releases, and GitHub Pages deployment.

Please ensure your code is well-tested (`cargo check` / `cargo clippy`) and adheres to clean standards before submitting a pull request!