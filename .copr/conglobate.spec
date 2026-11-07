Name:           conglobate
Version:        0.2.0~dev
Release:        1%{?buildtag}%{?dist}
Summary:        Image builder for the Pichi appliance

License:        Apache-2.0 AND MIT AND BSD-3-Clause AND Unicode-3.0 AND Zlib
URL:            https://github.com/pichi-vm/conglobate
Source0:        %{name}-%{version}.tar.gz
ExclusiveArch:  x86_64 aarch64

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  git
BuildRequires:  systemd-rpm-macros
Requires:       systemd
Requires:       systemd-networkd
Requires:       systemd-resolved
Requires:       e2fsprogs
Requires:       dosfstools
Requires:       git
Requires:       ca-certificates

%description
Image builder for the Pichi appliance.

%prep
%autosetup

%build
export CARGO_HOME="$PWD/.cargo-home"
cargo build --locked --release --bin conglobate --features cli

%install
install -Dpm0755 target/release/conglobate %{buildroot}%{_bindir}/conglobate
install -d %{buildroot}%{_unitdir}
install -pm0644 .copr/systemd/* %{buildroot}%{_unitdir}/

find .cargo-home -type f \( -iname 'license*' -o -iname 'licence*' -o \
    -iname 'copying*' -o -iname 'notice*' -o -iname 'copyright*' \) \
    -exec install -Dm0644 '{}' '%{buildroot}%{_licensedir}/%{name}/dependencies/{}' \;

%check
target/release/conglobate --help > /dev/null

# Installing the units does not enable or start the appliance.
%post
if [ -d /run/systemd/system ]; then systemctl daemon-reload || :; fi

%postun
if [ -d /run/systemd/system ]; then systemctl daemon-reload || :; fi

%files
%license %{_licensedir}/%{name}/dependencies/
%license LICENSE
%doc README.md
%{_bindir}/conglobate
%{_unitdir}/conglobate-build.service
%{_unitdir}/run-pichi-*.mount
