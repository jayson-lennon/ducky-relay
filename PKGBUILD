# Maintainer: Jayson Lennon <jayson@jaysonlennon.dev>

pkgname=duckycap
pkgver=0.1.0
pkgrel=3
pkgdesc='capture duckypad input and relay it to another application'
url=''
license=(GPL-3.0-only)
makedepends=('cargo')
depends=()
arch=('i686' 'x86_64' 'armv6h' 'armv7h')
source=()
b2sums=()

prepare() {
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --frozen --release --all-features
}

check() {
    export RUSTUP_TOOLCHAIN=stable
}

package() {
    install -Dm0755 -t "$pkgdir/usr/bin/" "target/release/duckycap"
    install -Dm0755 -t "$pkgdir/usr/bin/" "target/release/duckycap-varlink"
    # for custom license, e.g. MIT
    # install -Dm644 LICENSE "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
}
