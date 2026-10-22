{ config, pkgs, lib, ... }: {
 imports = [ /nixpkgs/nixos/modules/profiles/minimal.nix ];
 system.stateVersion = "26.05";
 boot.loader.grub.enable = false;
 boot.initrd.systemd.enable = false;
 boot.initrd.availableKernelModules = [ "virtio_pci" "virtio_blk" ];
 boot.kernelParams = [ "console=ttyS0" ];
 fileSystems."/" = { device = "/dev/vda"; fsType = "ext4"; };
 networking.hostName = "uml-validation";
 networking.useDHCP = false;
 environment.systemPackages = [ pkgs.acl pkgs.attr ];
 systemd.services.uml-validation = {
  wantedBy = [ "multi-user.target" ];
  after = [ "local-fs.target" ];
  serviceConfig = { Type = "oneshot"; StandardOutput = "journal+console"; };
  script = ''
   test "$(cat /nixos-derived)" = derived
   test "$(stat -c '%u:%g' /nixos-derived)" = 1234:2345
   test -e /etc/NIXOS
   echo UML_NIXOS_NATIVE_ROOT_BOOT_PASS > /dev/console
   ${pkgs.systemd}/bin/systemctl --no-block poweroff
  '';
 };
}
