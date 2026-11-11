# The version comes from build.sh: rpmbuild --define "shellman_version 0.1.0"

# rustc release builds carry no debug information
%global debug_package %{nil}

Name:           shellman
Version:        %{shellman_version}
Release:        %{?shellman_release}%{!?shellman_release:1}%{?dist}
Summary:        Embedded terminal for Nautilus (GNOME Files)
License:        GPL-3.0-or-later
URL:            https://github.com/scresto09/shellman
Source0:        shellman-%{version}.tar.gz

%if 0%{?suse_version}
BuildRequires:  AppStream
%else
BuildRequires:  appstream
%endif
BuildRequires:  gcc
BuildRequires:  gettext
BuildRequires:  make
BuildRequires:  pkgconfig(gtk4) >= 4.14
BuildRequires:  pkgconfig(libadwaita-1) >= 1.5
BuildRequires:  pkgconfig(libnautilus-extension-4)
BuildRequires:  pkgconfig(vte-2.91-gtk4)
Requires:       nautilus >= 43

%description
ShellMan adds a terminal at the bottom of Nautilus windows, one per tab. It
follows the displayed folder, the files selected in Nautilus are available in
the SEL variable, and terminals can be split. Press F4 to show or hide it.

%prep
%setup -q

%build
make PREFIX=%{_prefix}

%install
make install PREFIX=%{_prefix} DESTDIR=%{buildroot}
%find_lang %{name}

%check
make check

%files -f %{name}.lang
%license LICENSE
%doc README.md
%{_libdir}/nautilus/extensions-4/libnautilus-shellman.so
%{_datadir}/metainfo/io.github.scresto09.ShellMan.metainfo.xml
