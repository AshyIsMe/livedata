;; Development environment for livedata.
;; Usage: guix shell -D -f guix.scm
(use-modules (guix packages)
             (guix build-system cargo)
             (gnu packages rust)
             (gnu packages pkg-config)
             (gnu packages cmake)
             (gnu packages commencement)        ; gcc-toolchain
             (gnu packages freedesktop)          ; elogind (libsystemd)
             (gnu packages llvm)                 ; lld
             (gnu packages tls))                 ; openssl

(package
  (name "livedata")
  (version "0.0.0")
  (source #f)
  (build-system cargo-build-system)
  (native-inputs
   (list rust
         rust:cargo
         rust:clippy
         rustfmt
         pkg-config
         cmake
         gcc-toolchain
         lld
         elogind                                 ; provides libsystemd
         openssl))
  (synopsis "Live data streaming and analysis")
  (description
   "System observability without infrastructure overhead.
Collects journald logs and process metrics into DuckDB with a web UI.")
  (home-page "")
  (license #f))
