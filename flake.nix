{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    qahq.url = "github:mlavrinenko/qahq";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    # Typst-native static site generator that builds the docs under www/.
    tola.url = "github:tola-rs/tola-ssg/v0.7.1";
  };

  outputs =
    {
      flake-utils,
      naersk,
      nixpkgs,
      qahq,
      tola,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = (import nixpkgs) {
          inherit system;
        };

        naersk' = pkgs.callPackage naersk { };

      in
      {
        # For `nix build` & `nix run`. The same two flavours the release
        # publishes: `default` carries the Rust grammar alone, `full` carries
        # every one. Nix is the cheap place to offer the choice — no CI matrix
        # leg, and an overlay can override `cargoBuildOptions` for any subset in
        # between, which is the freedom a prebuilt asset cannot give.
        packages.default = naersk'.buildPackage {
          src = ./.;
        };

        packages.full = naersk'.buildPackage {
          src = ./.;
          cargoBuildOptions = prev: prev ++ [ "--features" "lang-all" ];
        };

        # For `nix develop`:
        devShells.default = pkgs.mkShell {
          # RUSTC_WRAPPER comes from the host session (global kache); the dev
          # shell inherits it and it never reaches the `nix build` sandbox.
          nativeBuildInputs = [
            qahq.packages.${system}.cargo-crap
            qahq.packages.${system}.ejectest
            qahq.packages.${system}.linecop
            qahq.packages.${system}.outdatty
            tola.packages.${system}.default
          ] ++ (with pkgs; [
            rustc
            cargo
            cargo-machete
            cargo-tarpaulin
            clippy
            rustfmt
            # `just check-changelog-history` compares CHANGELOG.md against what
            # each `v*` tag shipped, so git is a gate tool here and not merely
            # the thing the checkout arrived through. Pinned like the rest, so
            # the gate names `nixpkgs-tools` honestly.
            git
            jq
            just
            moreutils
            nixd
            pagefind
            rust-analyzer
            # tinymist bundles `typlite` (`$out/bin/typlite`) — the binary
            # docs/generate-md.sh runs to render the docs/src/*.typ sources into
            # the repo-root Markdown. Pulled in for that one binary, not for
            # tinymist's LSP; the closure is large but prebuilt in the nixpkgs
            # cache. Pin note: tinymist's embedded Typst must match the `typst`
            # below, or a source renders under one compiler and is queried
            # under another.
            tinymist
            typst
            # The Typst formatter `just fmt` runs after `cargo fmt`, so
            # editor-on-save and `just fmt-check` stay byte-identical.
            typstyle
            # flock, for the www/generated/ writers (see www/generate.sh).
            util-linux
            yq
          ]);
        };
      }
    );
}
