let pkgs = import /nixpkgs {}; in pkgs.runCommand "uml-compile-validation" { nativeBuildInputs = [ pkgs.stdenv.cc ]; } ''
 printf '#include <stdio.h>\nint main(void) { puts("UML_NIX_COMPILE_PASS"); return 0; }\n' > main.c
 cc main.c -o "$out"
 "$out"
''
