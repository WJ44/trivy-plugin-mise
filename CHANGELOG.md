# Changelog

## [1.1.0](https://github.com/WJ44/trivy-plugin-mise/compare/v1.0.0...v1.1.0) (2026-11-23)


### Features

* **sbom:** preserve toolchain dependencies and workflows in formulation mode ([#24](https://github.com/WJ44/trivy-plugin-mise/issues/24)) ([436fdff](https://github.com/WJ44/trivy-plugin-mise/commit/436fdffe51ee17a45e087022844b7c3943594e2f)), closes [#22](https://github.com/WJ44/trivy-plugin-mise/issues/22)
* **scan:** add tier-selection flags and tier headers ([#26](https://github.com/WJ44/trivy-plugin-mise/issues/26)) ([a08b70b](https://github.com/WJ44/trivy-plugin-mise/commit/a08b70b95a560c29882d0f61a67ccefb9a17ad7e)), closes [#23](https://github.com/WJ44/trivy-plugin-mise/issues/23)

## [1.0.0](https://github.com/WJ44/trivy-plugin-mise/compare/v0.1.0...v1.0.0) (2026-11-23)


### Features

* **cli:** add Rust project manifest, CLI skeleton, and CI test job ([557c495](https://github.com/WJ44/trivy-plugin-mise/commit/557c4958b85b867899c4a2d77d115d22241ad9fd)), closes [#2](https://github.com/WJ44/trivy-plugin-mise/issues/2)
* **cli:** add Rust project manifest, CLI skeleton, and CI test job ([#2](https://github.com/WJ44/trivy-plugin-mise/issues/2)) ([b9aa14f](https://github.com/WJ44/trivy-plugin-mise/commit/b9aa14f5bfcfd1b1dd492995cfd9cc1db6f797e5))
* **lockfile:** implement mise.lock parser and purl mapper ([#11](https://github.com/WJ44/trivy-plugin-mise/issues/11)) ([5930449](https://github.com/WJ44/trivy-plugin-mise/commit/59304497855ad5b53b5bf3e9e4526bd7f0933d00))
* **release:** add trivy plugin manifest and release workflow ([#7](https://github.com/WJ44/trivy-plugin-mise/issues/7)) ([#15](https://github.com/WJ44/trivy-plugin-mise/issues/15)) ([04b2943](https://github.com/WJ44/trivy-plugin-mise/commit/04b294325b4cd33287ccfad07aec4bd508d2aa61))
* **sbom:** implement sbom command with formulation and components modes ([#13](https://github.com/WJ44/trivy-plugin-mise/issues/13)) ([aa65bd4](https://github.com/WJ44/trivy-plugin-mise/commit/aa65bd4a865764cb7321dc987302a3950a836925))
* **scan:** implement scan command for repo and mise toolchain ([#14](https://github.com/WJ44/trivy-plugin-mise/issues/14)) ([201788b](https://github.com/WJ44/trivy-plugin-mise/commit/201788bdcd5ca96a3fab7a63902d730066d2f1e1))
* **sidecar:** extract transitive dependencies from .mise/locks ([#4](https://github.com/WJ44/trivy-plugin-mise/issues/4)) ([#12](https://github.com/WJ44/trivy-plugin-mise/issues/12)) ([5072f9a](https://github.com/WJ44/trivy-plugin-mise/commit/5072f9abba2e30adb0c88949b22cddda10a3854c))


### Bug Fixes

* **ci:** pin rustfmt and clippy components in mise.toml and test unimplemented exits ([f82c194](https://github.com/WJ44/trivy-plugin-mise/commit/f82c1945d6a741e723323702b040a87a0781abfd))
* **release:** use generic release-please updater for plugin.yaml ([ce80352](https://github.com/WJ44/trivy-plugin-mise/commit/ce8035220e74772dcb018f9f922c177d0e97ce24))


### Miscellaneous Chores

* prepare 1.0.0 release ([353cb3a](https://github.com/WJ44/trivy-plugin-mise/commit/353cb3ad2f7856fb0c748297b3d0cf8b0abc435d))
