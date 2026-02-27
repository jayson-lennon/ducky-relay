# Maintainer: Jayson Lennon <jayson@jaysonlennon.dev>

pkgname=duckycap
pkgver=0.1.0
pkgrel=6
pkgdesc='capture duckypad input and relay it to another application'
url=''
license=(GPL-3.0-only)
makedepends=('cargo')
depends=('systemd')
arch=('i686' 'x86_64' 'armv6h' 'armv7h')
source=(
    "99-duckypad.rules"
    "duckycap.service"
    "duckycap-varlink.service"
    "duckycap-varlink.socket"
    "README.md"
)
b2sums=(
    'SKIP'
    'SKIP'
    'SKIP'
    'SKIP'
    'SKIP'
)

# Build directory for isolated builds
_builddir="$srcdir/$pkgname-$pkgver"

prepare() {
    # Create build directory structure
    mkdir -p "$_builddir"
    
    # Copy source files to build directory
    cp -r "$startdir/src" "$_builddir/"
    cp "$startdir/Cargo.toml" "$_builddir/"
    cp "$startdir/Cargo.lock" "$_builddir/"
    
    # Copy systemd files to srcdir for package() access
    cp "$startdir/systemd/99-duckypad.rules" "$srcdir/"
    cp "$startdir/systemd/duckycap.service" "$srcdir/"
    cp "$startdir/systemd/duckycap-varlink.service" "$srcdir/"
    cp "$startdir/systemd/duckycap-varlink.socket" "$srcdir/"
    cp "$startdir/README.md" "$srcdir/"
    
    # Fetch dependencies in build directory
    cd "$_builddir"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "$_builddir"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR="$srcdir/$pkgname-$pkgver/build"
    cargo build --frozen --release --all-features
}

check() {
    cd "$_builddir"
    export RUSTUP_TOOLCHAIN=stable
}

package() {
    local _buildtarget="$srcdir/$pkgname-$pkgver/build/release"
    
    # Install binaries
    install -Dm0755 -t "$pkgdir/usr/bin/" "$_buildtarget/duckycap"
    install -Dm0755 -t "$pkgdir/usr/bin/" "$_buildtarget/duckycap-varlink"

    # Install udev rule
    install -Dm0644 -t "$pkgdir/usr/lib/udev/rules.d/" "$srcdir/99-duckypad.rules"

    # Install systemd units
    install -Dm0644 -t "$pkgdir/usr/lib/systemd/system/" "$srcdir/duckycap.service"
    install -Dm0644 -t "$pkgdir/usr/lib/systemd/system/" "$srcdir/duckycap-varlink.service"
    install -Dm0644 -t "$pkgdir/usr/lib/systemd/system/" "$srcdir/duckycap-varlink.socket"

    # Install documentation
    install -Dm0644 -t "$pkgdir/usr/share/doc/duckycap/" "$srcdir/README.md"
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
