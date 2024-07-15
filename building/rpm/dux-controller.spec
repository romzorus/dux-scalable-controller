Name:       dux-controller
Version:    PLACEHOLDER_FOR_VERSION
Release:    0
Summary:    Automation tool written in Rust
License:    Apache License, Version 2.0
URL:        https://gitlab.com/dux-tool/dux
Packager:   romzorus <romzorus591@gmail.com>


%description
A scalable and cloud-friendly automation / orchestration tool (controller component) written in Rust.

%install
mkdir -p %{buildroot}/usr/bin
cp dux-controller %{buildroot}/usr/bin/
mkdir -p %{buildroot}/etc/dux-controller
cp dux-controller.conf %{buildroot}/etc/dux-controller/

%files
/usr/bin/dux-controller
/etc/dux-controller/dux-controller.conf
