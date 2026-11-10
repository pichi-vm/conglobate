Name:           conglobate
Version:        0.2.0~dev
Release:        1%{?buildtag}%{?dist}
Summary:        Image builder with isolated command execution

License:        Apache-2.0 AND MIT AND BSD-3-Clause AND Unicode-3.0 AND Zlib
URL:            https://github.com/pichi-vm/conglobate
Source0:        %{name}-%{version}.tar.gz
ExclusiveArch:  x86_64 aarch64

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  git
Requires:       e2fsprogs
Requires:       dosfstools
Requires:       git
Requires:       ca-certificates

%description
Image builder with isolated command execution.

%prep
%autosetup

%build
export CARGO_HOME="$PWD/.cargo-home"
cargo build --locked --release --bin conglobate --features cli

%install
install -Dpm0755 target/release/conglobate %{buildroot}%{_bindir}/conglobate

find .cargo-home -type f \( -iname 'license*' -o -iname 'licence*' -o \
    -iname 'copying*' -o -iname 'notice*' -o -iname 'copyright*' \) \
    -exec install -Dm0644 '{}' '%{buildroot}%{_licensedir}/%{name}/dependencies/{}' \;

%check
target/release/conglobate --help > /dev/null

%files
%license %{_licensedir}/%{name}/dependencies/
%license LICENSE
%doc README.md
%{_bindir}/conglobate
