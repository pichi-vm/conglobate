Name: conglobate-validation-fixture
Version: %{fixture_version}
Release: 1
Summary: Bounded synthetic package-upgrade validation
License: Apache-2.0
BuildArch: noarch
AutoReqProv: no
%global __os_install_post %{nil}
%global __spec_install_post %{nil}
%global __check_files %{nil}

%description
Disposable fixture. Scriptlets only write fixture data inside the test root.

%install
mkdir -p %{buildroot}/usr/share/conglobate-upgrade %{buildroot}/etc
printf '%s\n' '%{version}' >%{buildroot}/usr/share/conglobate-upgrade/version
printf '%s\n' 'default-%{version}' >%{buildroot}/etc/conglobate-upgrade.conf

%pre
mkdir -p /var/log
printf '%s\n' "pre-%{version}-$1" >>/var/log/conglobate-upgrade-scriptlets

%post
printf '%s\n' "post-%{version}-$1" >>/var/log/conglobate-upgrade-scriptlets
mkdir -p /var/lib/conglobate-upgrade
printf '%s\n' '%{version}' >/var/lib/conglobate-upgrade/state
chown 1234:2345 /var/lib/conglobate-upgrade/state
chmod 640 /var/lib/conglobate-upgrade/state

%preun
printf '%s\n' "preun-%{version}-$1" >>/var/log/conglobate-upgrade-scriptlets

%postun
printf '%s\n' "postun-%{version}-$1" >>/var/log/conglobate-upgrade-scriptlets

%files
/usr/share/conglobate-upgrade/version
%config(noreplace) /etc/conglobate-upgrade.conf
