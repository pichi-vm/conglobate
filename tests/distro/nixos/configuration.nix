# SPDX-License-Identifier: Apache-2.0
{ pkgs, ... }: {
  imports = [ /nixpkgs/nixos/modules/profiles/minimal.nix ];
  system.stateVersion = "26.05";
  boot.loader.grub.enable = false;
  # This fixture packages the independently qualified Fedora/corium firmware.
  boot.kernel.enable = false;
  boot.initrd.enable = false;
  fileSystems."/" = { device = "none"; fsType = "ext4"; options = [ "ro" ]; };
  networking.hostName = "conglobate-nixos";
  networking.useDHCP = false;
  systemd.services.conglobate-qualification = {
    wantedBy = [ "multi-user.target" ];
    after = [ "local-fs.target" ];
    serviceConfig = { Type = "oneshot"; StandardOutput = "journal+console"; };
    path = [ pkgs.coreutils ];
    script = ''
      test "$(cat /conglobate-qualified)" = 'conglobate NixOS qualification'
      test "$(stat -c '%u:%g' /conglobate-qualified)" = 1234:2345
      test -e /etc/NIXOS
      test -L /run/current-system
      echo CONGLOBATE_NIXOS_SYSTEMD_BOOT_PASS > /dev/console
    '';
  };
}
