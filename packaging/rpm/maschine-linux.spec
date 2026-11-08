Name:           maschine-linux
Version:        0.9.2
Release:        1%{?dist}
Summary:        Native Instruments Maschine MK3 DAW & Controller integration for Linux
License:        GPL-3.0-or-later
URL:            https://github.com/timotheegrollier/maschine-linux
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  rust >= 1.75
BuildRequires:  cargo
BuildRequires:  alsa-lib-devel
BuildRequires:  systemd-devel
BuildRequires:  desktop-file-utils

Requires:       alsa-lib
Requires:       systemd-udev
Recommends:     pipewire
Recommends:     wireplumber

%description
Maschine Linux is an ultra-low-latency Digital Audio Workstation (DAW)
and controller host tailored specifically for the Native Instruments
Maschine MK3 and Mikro MK3 hardware controllers on Linux.

Features:
- Complete plug-and-play support for pads, knobs, wheels, and dual 480x272 screens
- Built-in 808 analog drum synthesis engine
- Polyphonic sampler and DSP guitar effects
- PipeWire / WirePlumber ultra-low-latency audio integration
- In-app automatic update system from GitHub releases

%prep
%autosetup

%build
cargo build --release --locked

%install
rm -rf %{buildroot}

# Install executable
install -D -p -m 0755 target/release/maschine-linux %{buildroot}%{_bindir}/maschine-linux

# Install desktop entry
install -D -p -m 0644 packaging/maschine-linux.desktop %{buildroot}%{_datadir}/applications/maschine-linux.desktop

# Install AppStream metadata
install -D -p -m 0644 packaging/io.github.timotheegrollier.maschine-linux.metainfo.xml %{buildroot}%{_datadir}/metainfo/io.github.timotheegrollier.maschine-linux.metainfo.xml

# Install icons
install -D -p -m 0644 packaging/icons/maschine-linux.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/maschine-linux.svg
for size in 16 32 48 64 128 256 512; do
  install -D -p -m 0644 packaging/icons/${size}x${size}/maschine-linux.png %{buildroot}%{_datadir}/icons/hicolor/${size}x${size}/apps/maschine-linux.png
done

# Install Udev rules (systemd standard location)
install -D -p -m 0644 packaging/udev/99-maschine-mk3.rules %{buildroot}%{_prefix}/lib/udev/rules.d/99-maschine-mk3.rules

# Install Realtime limits
install -D -p -m 0644 packaging/limits/99-realtime-audio.conf %{buildroot}%{_sysconfdir}/security/limits.d/99-realtime-audio.conf

# Install PipeWire / WirePlumber low-latency configs
install -D -p -m 0644 packaging/pipewire/99-maschine-lowlatency.conf %{buildroot}%{_datadir}/pipewire/pipewire.conf.d/99-maschine-lowlatency.conf
install -D -p -m 0644 packaging/wireplumber/51-maschine-mk3.conf %{buildroot}%{_datadir}/wireplumber/wireplumber.conf.d/51-maschine-mk3.conf

# Install systemd user service (for background daemon mode)
install -D -p -m 0644 packaging/systemd/maschine-linux.service %{buildroot}%{_prefix}/lib/systemd/user/maschine-linux.service

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/maschine-linux.desktop

%post
/usr/bin/udevadm control --reload-rules 2>/dev/null || :
/usr/bin/udevadm trigger --subsystem-match=usb --subsystem-match=hidraw 2>/dev/null || :
/usr/bin/update-desktop-database &>/dev/null || :
/usr/bin/touch --no-create %{_datadir}/icons/hicolor &>/dev/null || :
/usr/bin/gtk-update-icon-cache %{_datadir}/icons/hicolor &>/dev/null || :

%postun
/usr/bin/udevadm control --reload-rules 2>/dev/null || :
/usr/bin/udevadm trigger 2>/dev/null || :
/usr/bin/update-desktop-database &>/dev/null || :
/usr/bin/touch --no-create %{_datadir}/icons/hicolor &>/dev/null || :
/usr/bin/gtk-update-icon-cache %{_datadir}/icons/hicolor &>/dev/null || :

%files
%license LICENSE
%doc README.md CHANGELOG.md
%{_bindir}/maschine-linux
%{_datadir}/applications/maschine-linux.desktop
%{_datadir}/metainfo/io.github.timotheegrollier.maschine-linux.metainfo.xml
%{_datadir}/icons/hicolor/*/apps/maschine-linux.png
%{_datadir}/icons/hicolor/scalable/apps/maschine-linux.svg
%{_prefix}/lib/udev/rules.d/99-maschine-mk3.rules
%{_sysconfdir}/security/limits.d/99-realtime-audio.conf
%{_datadir}/pipewire/pipewire.conf.d/99-maschine-lowlatency.conf
%{_datadir}/wireplumber/wireplumber.conf.d/51-maschine-mk3.conf
%{_prefix}/lib/systemd/user/maschine-linux.service

%changelog
* Sun Nov 08 2026 timotheegrollier <timo@example.com> - 0.9.2-1
- Restore MK3 project browser access, improve mute pad LED visibility, and add restart button after auto-update

* Sun Nov 08 2026 timotheegrollier <timo@example.com> - 0.9.1-1
- Fix MK3 FILE SAVE workflow and project browser feedback, add Maschine-style per-pad mute with dimmed LEDs, and record Keyboard/Chords with actual MIDI pitch during REC

* Sun Nov 08 2026 timotheegrollier <timo@example.com> - 0.9.0-1
- Release 0.9.0 with FILE/project browser and quick save (Shift+FILE SAVE), per-pad volume/pan per Group in mixer and MK3, and repaired Keyboard/Chords with scale/root/octave/chord controls and dedicated screens

* Sat Nov 07 2026 timotheegrollier <timo@example.com> - 0.8.0-1
- Release 0.8.0 with complete Pattern workflow (create, duplicate, clear, delete, rename, slot navigation), hideable GUI panels, and graceful MK3 shutdown that restores the default logo and clears pad LEDs

* Sat Nov 07 2026 timotheegrollier <timo@example.com> - 0.7.0-1
- Release 0.7.0 with latched MK3 navigation, unified PLUGIN view, TB-303 simple mode, per-Sound polyphony/choke, responsive GUI, and standalone --no-hardware mode

* Sun Nov 01 2026 timotheegrollier <timo@example.com> - 0.6.0-1
- Release 0.6.0 with GUI studio interface, 808 analog synthesis, and auto-updater
