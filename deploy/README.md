# Production deploy templates

This directory contains templates for a production HomePanel install. They are
meant to be copied into place on the target host and adjusted for that host.

## Files

- `config.example.toml`: example runtime config for `/etc/homepanel/config.toml`
- `homepanel.service`: systemd unit template for the `homepaneld` daemon

## Suggested install flow

1. Build the release binary.

   ```bash
   cargo build --release -p homepaneld
   ```

2. Create the config directory and copy the example config.

   ```bash
   sudo install -d /etc/homepanel
   sudo install -m 0644 deploy/config.example.toml /etc/homepanel/config.toml
   ```

3. Update `/etc/homepanel/config.toml` for the real host.

   - Set `server.public_url` to the real HTTP or HTTPS URL.
   - Keep `data.data_dir` at `/var/lib/homepanel` unless you need a custom path.
   - Keep `data.database_url` pointed at the SQLite file in `/var/lib/homepanel`.

4. Install the systemd unit.

   ```bash
   sudo install -m 0644 deploy/homepanel.service /etc/systemd/system/homepanel.service
   sudo systemctl daemon-reload
   sudo systemctl enable --now homepanel.service
   ```

## Notes

- The service template uses `superadmin` as both user and group, matching the
  current production setup described in the repo.
- The unit expects the release binary at:

  ```text
  /home/superadmin/projects/homepanel/target/release/homepaneld
  ```

- `StateDirectory=homepanel` and `RuntimeDirectory=homepanel` let systemd manage
  `/var/lib/homepanel` and `/run/homepanel` without hardcoding extra paths in
  the unit.
- These files are templates only. Do not treat them as the live production
  config without checking host-specific values first.
