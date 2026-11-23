# Changelog

## [0.1.1](https://github.com/WJ44/trivy-plugin-mise/compare/v0.1.0...v0.1.1) (2026-11-23)


### Features

* **cli:** add Rust project manifest, CLI skeleton, and CI test job ([a2c2272](https://github.com/WJ44/trivy-plugin-mise/commit/a2c227213113104ad54a01c1dc398ca649cc6275)), closes [#2](https://github.com/WJ44/trivy-plugin-mise/issues/2)
* **cli:** add Rust project manifest, CLI skeleton, and CI test job ([#2](https://github.com/WJ44/trivy-plugin-mise/issues/2)) ([08a73b9](https://github.com/WJ44/trivy-plugin-mise/commit/08a73b91bf271ba8e3f547b43a4e34f38d3fab1d))
* **lockfile:** implement mise.lock parser and purl mapper ([#11](https://github.com/WJ44/trivy-plugin-mise/issues/11)) ([c424e46](https://github.com/WJ44/trivy-plugin-mise/commit/c424e4696f0cc98a1055c1ee2f3294e304f3edb3))
* **release:** add trivy plugin manifest and release workflow ([#7](https://github.com/WJ44/trivy-plugin-mise/issues/7)) ([#15](https://github.com/WJ44/trivy-plugin-mise/issues/15)) ([7d8e6d7](https://github.com/WJ44/trivy-plugin-mise/commit/7d8e6d7e4924f3900c174a0df795faa41f824877))
* **sbom:** implement sbom command with formulation and components modes ([#13](https://github.com/WJ44/trivy-plugin-mise/issues/13)) ([a9a55d4](https://github.com/WJ44/trivy-plugin-mise/commit/a9a55d484e8b9700fb10c6af0ff39df374cfc505))
* **scan:** implement scan command for repo and mise toolchain ([#14](https://github.com/WJ44/trivy-plugin-mise/issues/14)) ([68b2a9d](https://github.com/WJ44/trivy-plugin-mise/commit/68b2a9dcd39e9c1b50dc2c0ae3522e67696c8842))
* **sidecar:** extract transitive dependencies from .mise/locks ([#4](https://github.com/WJ44/trivy-plugin-mise/issues/4)) ([#12](https://github.com/WJ44/trivy-plugin-mise/issues/12)) ([29d43c7](https://github.com/WJ44/trivy-plugin-mise/commit/29d43c70c4f97871eb7294f0397facdd9a42a66a))


### Bug Fixes

* **ci:** pin rustfmt and clippy components in mise.toml and test unimplemented exits ([1ba29fe](https://github.com/WJ44/trivy-plugin-mise/commit/1ba29fe9ba63977ca1be098ad4996451a477c724))
