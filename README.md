# mender-bootc

`mender-bootc` is a [Mender Update Module](https://docs.mender.io/artifact-creation/create-a-custom-update-module)
that updates a [bootc](https://bootc-dev.github.io/bootc/) system using the
[composefs backend](https://bootc.dev/bootc/bootc-experimental-composefs.7.html).

## Adding the Update Module to an image

For the `mender-bootc` Update Module to function, the bootc image requires the
Mender Client (`mender-update`), the Update Module at
`/usr/share/mender/modules/v3/bootc`, and a device type in
`/var/lib/mender/device_type`. See
[`tests/images/fedora/Containerfile`](tests/images/fedora/Containerfile) for an
example.

```dockerfile
RUN cargo install --locked --git https://github.com/yeetypete/mender-bootc \
        --bin mender-bootc --root /out
# In the final stage:
COPY --from=builder /out/bin/mender-bootc /usr/share/mender/modules/v3/bootc
```

## Creating an Artifact

[`bootc-artifact-gen`](src/artifact_gen.rs) is a CLI that generates a
[Mender Artifact](https://docs.mender.io/overview/artifact) for the
`mender-bootc` Update Module from a container image. It requires
[`mender-artifact`](https://docs.mender.io/downloads#mender-artifact) and
[`skopeo`](https://github.com/containers/skopeo) to be installed:

```bash
cargo install --locked --git https://github.com/yeetypete/mender-bootc --bin bootc-artifact-gen

bootc-artifact-gen --artifact-name system-v1 --compatible-types my-device \
    --output-path system-v1.mender docker.io/example/my-bootc-image:v1
```

The Artifact holds the image reference pinned by digest, and the device pulls
the image from the registry. Options after `--` go to
`mender-artifact write module-image`, e.g. `-- --key key.pem` to sign.

## Deploying

Upload the Artifact to a Mender Server, or install it in
[standalone mode](https://docs.mender.io/artifact-creation/standalone-deployment):

```bash
sudo mender-update install system-v1.mender
sudo reboot
sudo mender-update commit  # Or rollback.
```

## How it works

The Mender Client runs the `mender-bootc` Update Module for each state of an
update, following the
[Update Modules v3 protocol](https://github.com/mendersoftware/mender/blob/master/Documentation/update-modules-v3-file-api.md).
The Update Module simply uses bootc commands as its backend: under the hood
it stages the Artifact's image with
[`bootc switch`](https://bootc.dev/bootc/man/bootc-switch.8.html), and on
failure cancels the staged deployment or rolls back with
[`bootc rollback`](https://bootc.dev/bootc/man/bootc-rollback.8.html).

The `mender-bootc` Update Module implementation is stateless, using
[`bootc status`](https://bootc.dev/bootc/man/bootc-status.8.html) output
to track deployments.

## Development

```bash
just check             # Check formatting and lint.
just test              # Run the unit tests.
just test-integration  # Run the integration tests in bcvk VMs.
```

The integration tests require [Podman](https://podman.io/),
[bcvk](https://github.com/bootc-dev/bcvk) with `libvirt`, and `mender-artifact`.

## License

`mender-bootc` is released under the [MIT License](LICENSE).
