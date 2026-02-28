# Maintainer: Jayson Lennon <jayson@jaysonlennon.dev>

pkgname=duckycap
pkgver=0.3.0
pkgrel=1
pkgdesc='capture duckypad input and relay it to another application'
url=''
license=(GPL-3.0-only)
makedepends=('cargo')
depends=('systemd')
arch=('i686' 'x86_64' 'armv6h' 'armv7h')

# No source array needed - we reference files directly from $startdir
# This avoids conflicts with the project's src/ directory

# Dedicated build directory outside of project's src/ folder
_builddir="$startdir/.build/$pkgname-$pkgver"

prepare() {
    # Create dedicated build directory
    rm -rf "$_builddir"
    mkdir -p "$_builddir"

    # Copy Rust source files to build directory
    cp -r "$startdir/src" "$_builddir/"
    cp "$startdir/Cargo.toml" "$_builddir/"
    cp "$startdir/Cargo.lock" "$_builddir/"

    # Fetch dependencies in build directory
    cd "$_builddir"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "$_builddir"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR="$_builddir/target"
    cargo build --frozen --release --all-features
}

check() {
    cd "$_builddir"
    export RUSTUP_TOOLCHAIN=stable
}

package() {
    local _buildtarget="$_builddir/target/release"

    # Install binaries
    install -Dm0755 -t "$pkgdir/usr/bin/" "$_buildtarget/duckycap"
    install -Dm0755 -t "$pkgdir/usr/bin/" "$_buildtarget/duckycap-varlink"

    # Install udev rule (reference directly from project's systemd folder)
    install -Dm0644 -t "$pkgdir/usr/lib/udev/rules.d/" "$startdir/systemd/99-duckypad.rules"

    # Install systemd units (reference directly from project's systemd folder)
    install -Dm0644 -t "$pkgdir/usr/lib/systemd/system/" "$startdir/systemd/duckycap.service"
    install -Dm0644 -t "$pkgdir/usr/lib/systemd/system/" "$startdir/systemd/duckycap-varlink.service"
    install -Dm0644 -t "$pkgdir/usr/lib/systemd/system/" "$startdir/systemd/duckycap-varlink.socket"

    # Install documentation (reference directly from project folder)
    install -Dm0644 -t "$pkgdir/usr/share/doc/duckycap/" "$startdir/README.md"
}

post_install() {
    # Reload udev rules
    udevadm control --reload-rules
    udevadm trigger

    # Reload systemd daemon
    systemctl daemon-reload

    # Enable and start the varlink socket
    systemctl enable --now duckycap-varlink.socket
}

post_upgrade() {
    # Reload udev rules
    udevadm control --reload-rules
    udevadm trigger

    # Reload systemd daemon
    systemctl daemon-reload
}

post_remove() {
    # Stop and disable services
    systemctl stop duckycap.service 2>/dev/null || true
    systemctl stop duckycap-varlink.socket 2>/dev/null || true
    systemctl disable duckycap.service 2>/dev/null || true
    systemctl disable duckycap-varlink.socket 2>/dev/null || true

    # Reload systemd daemon
    systemctl daemon-reload

    # Reload udev rules
    udevadm control --reload-rules
}
