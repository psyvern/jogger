with (import <nixpkgs> {});
let
  libPath = lib.makeLibraryPath [ glib gtk4 gtk4-layer-shell pango ];
in
mkShell {
  buildInputs = [
    adwaita-icon-theme
    gdk-pixbuf
    glib
    gtk4-layer-shell
    glib-networking
    shared-mime-info
    hicolor-icon-theme
    gsettings-desktop-schemas
    libxkbcommon
    gtk4
  ];

  nativeBuildInputs = [
    gobject-introspection
    gtk4
    pkg-config
  ];

  propagatedBuildInputs = [
    gtk4
  ];

  shellHook = ''
     export LD_LIBRARY_PATH="''${LD_LIBRARY_PATH:+:}${libPath}"
  '';
}
